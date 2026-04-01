//! NFT module — fetch and decode standard NFTs (Token Metadata) and
//! compressed NFTs (Bubblegum / Helius DAS) for a given wallet owner.
//!
//! **Standard NFTs:**
//!   1. getTokenAccountsByOwner → filter: decimals=0, amount=1
//!   2. Derive Metadata PDA manually (seeds: ["metadata", MPL_PROGRAM_ID, mint])
//!   3. getAccountInfo on PDA → parse Borsh-encoded MetadataV3
//!   4. HTTP-fetch off-chain JSON from the `uri` field
//!
//! **Compressed NFTs (cNFTs):**
//!   1. Helius DAS `getAssetsByOwner` → returns full metadata inline
//!
//! We avoid importing mpl-token-metadata as a Cargo dep to prevent the
//! zeroize version conflict it introduces. Instead we derive the PDA manually
//! and parse the on-chain account with a compact Borsh layout.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{commitment_config::CommitmentConfig, pubkey::Pubkey};
use std::str::FromStr;

// Helius API key baked in — can be changed at runtime in future
const HELIUS_API_KEY: &str = "5bb5fed2-8d33-458b-b7d2-3d18fdbb3da5";
const HELIUS_RPC: &str = "https://mainnet.helius-rpc.com";

/// Metaplex Token Metadata program ID
const MPL_TOKEN_METADATA_PROGRAM: &str = "metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s";

// ─── Public types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NftAttribute {
    pub trait_type: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NftMetadata {
    pub mint: String,
    pub name: String,
    pub symbol: String,
    pub uri: String,
    pub image_url: String,
    pub description: String,
    pub attributes: Vec<NftAttribute>,
    pub is_compressed: bool,
    pub collection: Option<String>,
}

// ─── Off-chain JSON ────────────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
struct OffChainMetadata {
    #[serde(default)]
    image: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    attributes: Vec<OffChainAttribute>,
    #[serde(default)]
    collection: Option<OffChainCollection>,
}

#[derive(Deserialize)]
struct OffChainAttribute {
    trait_type: String,
    value: serde_json::Value,
}

#[derive(Deserialize)]
struct OffChainCollection {
    name: Option<String>,
}

// ─── PDA derivation ───────────────────────────────────────────────────────────

/// Derive the Token Metadata PDA for a given mint.
/// Seeds: [b"metadata", mpl_token_metadata_program_id, mint_pubkey]
fn get_metadata_pda(mint: &Pubkey) -> Pubkey {
    let mpl_program_id =
        Pubkey::from_str(MPL_TOKEN_METADATA_PROGRAM).unwrap();
    let seeds: &[&[u8]] = &[b"metadata", mpl_program_id.as_ref(), mint.as_ref()];
    Pubkey::find_program_address(seeds, &mpl_program_id).0
}

/// Parse the name, symbol, and uri from the raw on-chain Metadata account data.
/// The Metaplex MetadataV3 Borsh layout (simplified):
///   1 byte  key discriminator
///   32 bytes update_authority
///   32 bytes mint
///   4+N bytes name (u32 len + UTF-8 bytes + null-padding to 36 bytes)
///   4+N bytes symbol (u32 len + UTF-8 + null-pad to 14 bytes)
///   4+N bytes uri (u32 len + UTF-8 + null-pad to 204 bytes)
///   ... rest: seller_fee, creators, collection, uses, etc.
fn parse_metadata_name_symbol_uri(data: &[u8]) -> Option<(String, String, String)> {
    let mut cursor = 0usize;
    // Skip: key(1) + update_authority(32) + mint(32) = 65
    cursor += 65;
    if data.len() < cursor + 4 {
        return None;
    }
    // name: u32 little-endian length + bytes (padded to 36)
    let name_len = u32::from_le_bytes([
        data[cursor], data[cursor + 1], data[cursor + 2], data[cursor + 3],
    ]) as usize;
    cursor += 4;
    if data.len() < cursor + name_len {
        return None;
    }
    let name = String::from_utf8_lossy(&data[cursor..cursor + name_len])
        .trim_matches('\0').to_string();
    // Borsh-encoded strings in Metaplex are NOT padded — they're exactly len bytes
    cursor += name_len;

    // symbol: u32 len + bytes
    if data.len() < cursor + 4 {
        return None;
    }
    let sym_len = u32::from_le_bytes([
        data[cursor], data[cursor + 1], data[cursor + 2], data[cursor + 3],
    ]) as usize;
    cursor += 4;
    if data.len() < cursor + sym_len {
        return None;
    }
    let symbol = String::from_utf8_lossy(&data[cursor..cursor + sym_len])
        .trim_matches('\0').to_string();
    cursor += sym_len;

    // uri: u32 len + bytes
    if data.len() < cursor + 4 {
        return None;
    }
    let uri_len = u32::from_le_bytes([
        data[cursor], data[cursor + 1], data[cursor + 2], data[cursor + 3],
    ]) as usize;
    cursor += 4;
    if data.len() < cursor + uri_len {
        return None;
    }
    let uri = String::from_utf8_lossy(&data[cursor..cursor + uri_len])
        .trim_matches('\0').to_string();

    Some((name, symbol, uri))
}

// ─── Handler ──────────────────────────────────────────────────────────────────

pub struct NftHandler;

impl NftHandler {
    /// Fetch standard (uncompressed) NFTs owned by `owner_pubkey_str`.
    pub fn fetch_nfts(owner_pubkey_str: &str) -> Result<Vec<NftMetadata>> {
        let helius_url = format!("{}/?api-key={}", HELIUS_RPC, HELIUS_API_KEY);
        let rpc = RpcClient::new_with_commitment(helius_url, CommitmentConfig::confirmed());
        let owner = Pubkey::from_str(owner_pubkey_str)
            .map_err(|e| anyhow!("Invalid pubkey: {}", e))?;

        let token_accounts = rpc
            .get_token_accounts_by_owner(
                &owner,
                solana_client::rpc_request::TokenAccountsFilter::ProgramId(spl_token::id()),
            )
            .map_err(|e| anyhow!("RPC error: {}", e))?;

        let http = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()?;

        let mut nfts = Vec::new();

        for account in &token_accounts {
            let ui_account = &account.account;
            let parsed = match &ui_account.data {
                solana_account_decoder::UiAccountData::Json(p) => p,
                _ => continue,
            };
            let info = match parsed.parsed.get("info") {
                Some(v) => v,
                None => continue,
            };

            let decimals = info
                .get("tokenAmount")
                .and_then(|ta| ta.get("decimals"))
                .and_then(|d| d.as_u64())
                .unwrap_or(1);
            let amount_str = info
                .get("tokenAmount")
                .and_then(|ta| ta.get("amount"))
                .and_then(|a| a.as_str())
                .unwrap_or("0");
            let amount: u64 = amount_str.parse().unwrap_or(0);

            if decimals != 0 || amount != 1 {
                continue; // not an NFT
            }

            let mint_str = match info.get("mint").and_then(|m| m.as_str()) {
                Some(m) => m,
                None => continue,
            };
            let mint = match Pubkey::from_str(mint_str) {
                Ok(m) => m,
                Err(_) => continue,
            };

            let metadata_pda = get_metadata_pda(&mint);
            let account_data = match rpc.get_account_data(&metadata_pda) {
                Ok(d) => d,
                Err(_) => continue,
            };

            let (name, symbol, uri) = match parse_metadata_name_symbol_uri(&account_data) {
                Some(t) => t,
                None => continue,
            };

            let (image_url, description, attributes, collection) =
                Self::fetch_off_chain_metadata(&http, &uri);

            nfts.push(NftMetadata {
                mint: mint_str.to_string(),
                name,
                symbol,
                uri,
                image_url,
                description,
                attributes,
                is_compressed: false,
                collection,
            });
        }

        Ok(nfts)
    }

    /// Fetch compressed NFTs (cNFTs) using Helius DAS `getAssetsByOwner`.
    pub fn fetch_compressed_nfts(owner_pubkey_str: &str) -> Result<Vec<NftMetadata>> {
        let url = format!("{}/?api-key={}", HELIUS_RPC, HELIUS_API_KEY);
        let http = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()?;

        let payload = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "wallet-nft-fetch",
            "method": "getAssetsByOwner",
            "params": {
                "ownerAddress": owner_pubkey_str,
                "page": 1,
                "limit": 100,
                "displayOptions": {
                    "showFungible": false,
                    "showCollectionMetadata": true
                }
            }
        });

        let res: serde_json::Value = http
            .post(&url)
            .json(&payload)
            .send()
            .map_err(|e| anyhow!("Helius request failed: {}", e))?
            .json()
            .map_err(|e| anyhow!("Helius response parse failed: {}", e))?;

        let assets = match res
            .get("result")
            .and_then(|r| r.get("items"))
            .and_then(|i| i.as_array())
        {
            Some(a) => a.clone(),
            None => return Ok(vec![]),
        };

        let mut nfts = Vec::new();
        for asset in &assets {
            let is_compressed = asset
                .get("compression")
                .and_then(|c| c.get("compressed"))
                .and_then(|c| c.as_bool())
                .unwrap_or(false);

            if !is_compressed {
                continue;
            }

            let mint = asset
                .get("id")
                .and_then(|i| i.as_str())
                .unwrap_or("")
                .to_string();

            let name = asset
                .pointer("/content/metadata/name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let symbol = asset
                .pointer("/content/metadata/symbol")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let description = asset
                .pointer("/content/metadata/description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let image_url = asset
                .pointer("/content/links/image")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let uri = asset
                .pointer("/content/json_uri")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let attributes = asset
                .pointer("/content/metadata/attributes")
                .and_then(|a| a.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|a| {
                            let trait_type = a
                                .get("trait_type")
                                .and_then(|t| t.as_str())
                                .unwrap_or("")
                                .to_string();
                            let value = a.get("value").map(|v| {
                                let s = v.to_string();
                                s.trim_matches('"').to_string()
                            }).unwrap_or_default();
                            if trait_type.is_empty() {
                                None
                            } else {
                                Some(NftAttribute { trait_type, value })
                            }
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            let collection = asset
                .pointer("/grouping")
                .and_then(|g| g.as_array())
                .and_then(|arr| {
                    arr.iter().find(|g| {
                        g.get("group_key")
                            .and_then(|k| k.as_str())
                            .unwrap_or("") == "collection"
                    })
                })
                .and_then(|g| g.get("group_value"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            nfts.push(NftMetadata {
                mint,
                name,
                symbol,
                uri,
                image_url,
                description,
                attributes,
                is_compressed: true,
                collection,
            });
        }

        Ok(nfts)
    }

    /// Fetch all NFTs (standard + compressed).
    pub fn fetch_all_nfts(owner_pubkey_str: &str) -> Result<Vec<NftMetadata>> {
        let mut all = Self::fetch_nfts(owner_pubkey_str)?;
        let compressed = Self::fetch_compressed_nfts(owner_pubkey_str)?;
        all.extend(compressed);
        Ok(all)
    }

    // ─── Private helpers ───────────────────────────────────────────────────────

    fn fetch_off_chain_metadata(
        http: &reqwest::blocking::Client,
        uri: &str,
    ) -> (String, String, Vec<NftAttribute>, Option<String>) {
        if uri.is_empty() {
            return (String::new(), String::new(), vec![], None);
        }
        let res = match http.get(uri).send() {
            Ok(r) => r,
            Err(_) => return (String::new(), String::new(), vec![], None),
        };
        let json: OffChainMetadata = match res.json() {
            Ok(j) => j,
            Err(_) => return (String::new(), String::new(), vec![], None),
        };
        let attributes = json.attributes.into_iter().map(|a| NftAttribute {
            trait_type: a.trait_type,
            value: {
                let s = a.value.to_string();
                s.trim_matches('"').to_string()
            },
        }).collect();
        let collection_name = json.collection.and_then(|c| c.name);
        (json.image, json.description, attributes, collection_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metadata_pda_derivation() {
        // DeGods collection mint — known PDA address
        let mint =
            Pubkey::from_str("6XxjKYFbcndh2gDcsUrmZgVEsoDxXMnfsaGY6fpTJzNr").unwrap();
        let pda = get_metadata_pda(&mint);
        assert_ne!(pda, Pubkey::default());
        println!("Metadata PDA: {}", pda);
    }

    #[test]
    fn test_invalid_pubkey_returns_error() {
        let result = NftHandler::fetch_nfts("not_a_valid_pubkey");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_metadata_too_short() {
        let data = vec![0u8; 10];
        let result = parse_metadata_name_symbol_uri(&data);
        assert!(result.is_none());
    }
}
