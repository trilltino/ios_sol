import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

interface NftAttribute {
  trait_type: string;
  value: string;
}

interface NftMetadata {
  mint: string;
  name: string;
  symbol: string;
  uri: string;
  image_url: string;
  description: string;
  attributes: NftAttribute[];
  is_compressed: boolean;
  collection: string | null;
}

interface Props {
  pubkey: string;
}

export function NFTGallery({ pubkey }: Props) {
  const [nfts, setNfts] = useState<NftMetadata[]>([]);
  const [loading, setLoading] = useState(false);
  const [selected, setSelected] = useState<NftMetadata | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (pubkey) loadNfts();
  }, [pubkey]);

  async function loadNfts() {
    setLoading(true);
    setError(null);
    try {
      const result = await invoke<NftMetadata[]>("get_all_nfts", { pubkey });
      setNfts(result);
    } catch (e) {
      setError(e as string);
    } finally {
      setLoading(false);
    }
  }

  if (loading) {
    return (
      <div className="nft-loading">
        <div className="nft-spinner" />
        <p>Loading NFTs…</p>
      </div>
    );
  }

  if (error) {
    return (
      <div className="nft-error">
        <p>⚠ {error}</p>
        <button className="nft-retry-btn" onClick={loadNfts}>Retry</button>
      </div>
    );
  }

  if (nfts.length === 0) {
    return (
      <div className="nft-empty">
        <div className="nft-empty-icon">🖼</div>
        <p>No NFTs found in this wallet</p>
        <button className="nft-retry-btn" onClick={loadNfts}>Refresh</button>
      </div>
    );
  }

  return (
    <>
      {/* NFT Grid */}
      <div className="nft-header">
        <span className="nft-count">{nfts.length} NFT{nfts.length !== 1 ? "s" : ""}</span>
        <button className="nft-retry-btn nft-refresh-btn" onClick={loadNfts} title="Refresh">↻</button>
      </div>
      <div className="nft-grid">
        {nfts.map((nft) => (
          <div
            key={nft.mint}
            className="nft-card"
            onClick={() => setSelected(nft)}
          >
            <div className="nft-image-wrap">
              {nft.image_url ? (
                <img
                  src={nft.image_url}
                  alt={nft.name}
                  className="nft-image"
                  loading="lazy"
                  onError={(e) => {
                    (e.target as HTMLImageElement).src =
                      "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='100' height='100'%3E%3Crect width='100' height='100' fill='%23222'/%3E%3Ctext x='50' y='55' font-size='30' text-anchor='middle' fill='%23555'%3E🖼%3C/text%3E%3C/svg%3E";
                  }}
                />
              ) : (
                <div className="nft-placeholder">🖼</div>
              )}
              {nft.is_compressed && (
                <span className="nft-compressed-badge">cNFT</span>
              )}
            </div>
            <div className="nft-card-info">
              <div className="nft-name" title={nft.name}>
                {nft.name || "Unnamed"}
              </div>
              {nft.collection && (
                <div className="nft-collection">{nft.collection}</div>
              )}
            </div>
          </div>
        ))}
      </div>

      {/* Detail Sheet */}
      {selected && (
        <div className="nft-detail-overlay" onClick={() => setSelected(null)}>
          <div className="nft-detail-sheet" onClick={(e) => e.stopPropagation()}>
            <button className="nft-close-btn" onClick={() => setSelected(null)}>✕</button>
            {selected.image_url && (
              <img
                src={selected.image_url}
                alt={selected.name}
                className="nft-detail-image"
              />
            )}
            <div className="nft-detail-body">
              <h2 className="nft-detail-name">{selected.name || "Unnamed"}</h2>
              {selected.collection && (
                <div className="nft-detail-collection">
                  <span className="nft-collection-dot" />
                  {selected.collection}
                </div>
              )}
              {selected.is_compressed && (
                <span className="nft-detail-compressed-badge">Compressed NFT</span>
              )}
              {selected.description && (
                <p className="nft-detail-desc">{selected.description}</p>
              )}
              {selected.attributes.length > 0 && (
                <>
                  <div className="nft-attrs-title">Attributes</div>
                  <div className="nft-attrs-grid">
                    {selected.attributes.map((attr, i) => (
                      <div key={i} className="nft-attr">
                        <div className="nft-attr-type">{attr.trait_type}</div>
                        <div className="nft-attr-value">{attr.value}</div>
                      </div>
                    ))}
                  </div>
                </>
              )}
              <div className="nft-mint-row">
                <span className="nft-mint-label">Mint</span>
                <span className="nft-mint-value" title={selected.mint}>
                  {selected.mint.slice(0, 8)}…{selected.mint.slice(-6)}
                </span>
              </div>
            </div>
          </div>
        </div>
      )}
    </>
  );
}
