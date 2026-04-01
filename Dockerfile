# Use the official Rust image
FROM rust:1.77-bullseye

# Install Node.js and npm
RUN curl -fsSL https://deb.nodesource.com/setup_20.x | bash - \
    && apt-get install -y nodejs

# Install Tauri Linux dependencies for headless builds
RUN apt-get update && apt-get install -y \
    libwebkit2gtk-4.1-dev \
    build-essential \
    curl \
    wget \
    file \
    libxdo-dev \
    libssl-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev \
    && rm -rf /var/lib/apt/lists/*

# Set up the application directory
WORKDIR /app

# Copy the entire workspace
COPY . .

# Install frontend dependencies
WORKDIR /app/crates/ios-app
RUN npm install

# Build the Tauri app in headless release mode
# This will output the Linux bundle directly mapped out through volumes
CMD ["npm", "run", "tauri", "build"]
