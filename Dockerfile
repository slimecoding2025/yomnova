# YOMNOVA web server + Playground, one image: Node serves the site and the API, and runs the real `yomnova` binary.
# Build:  docker build -t yomnova .      Run:  docker run --rm -p 3000:3000 yomnova
# NOTE: there is no OS-level sandbox inside the server itself; this container is the isolation boundary.

# ---- 1. Compiler (Rust) -----------------------------------------------------
FROM rust:1-slim-bookworm AS compiler
WORKDIR /src
COPY yomnova/ ./yomnova/
RUN cargo build --release --manifest-path yomnova/Cargo.toml -p yomnova-cli

# ---- 2. Web bundle (Vite) ---------------------------------------------------
FROM node:22-bookworm-slim AS web
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci
COPY index.html vite.config.ts tsconfig.json ./
COPY public ./public
COPY src ./src
RUN npm run build

# ---- 3. Runtime -------------------------------------------------------------
FROM node:22-bookworm-slim
ENV NODE_ENV=production
WORKDIR /app
COPY --from=web /app/node_modules ./node_modules
COPY package.json tsconfig.json server.ts ./
COPY server ./server
COPY --from=web /app/dist ./dist
# Documentation and examples: the Playground lists the examples; the optional AI Assistant retrieves from the docs.
COPY README.md ./
COPY docs ./docs
COPY yomnova/examples ./yomnova/examples
COPY --from=compiler /src/yomnova/target/release/yomnova ./yomnova/target/release/yomnova

USER node
# Render and most hosts set PORT; the server reads it (default 3000).
EXPOSE 3000
CMD ["node_modules/.bin/tsx", "server.ts", "--prod"]
