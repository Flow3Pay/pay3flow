# Keep these deps/builder stages aligned with the frontend Dockerfile so an
# existing application build can be reused when only runtime packaging changes.
FROM node:24-alpine AS deps
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci --ignore-scripts

FROM deps AS builder
WORKDIR /app
COPY . .
RUN NODE_OPTIONS=--max-old-space-size=6144 npm run build

FROM deps AS runtime-deps
# Published React icons contain their SVG paths. Their raw font-source packages,
# TypeScript declarations and source maps are not used by the Node runtime.
# Copy a filtered tree to a fresh directory; the runner does not inherit the
# full dependency layer (deleting files in an inherited layer would not shrink it).
RUN set -o pipefail; \
    mkdir -p /runtime; \
    tar -C /app \
      --exclude='*/@material-symbols/svg-*' \
      --exclude='*.d.ts' --exclude='*.d.mts' --exclude='*.d.cts' \
      --exclude='*.map' --exclude='*.tsbuildinfo' \
      -cf - node_modules | tar -C /runtime -xf -

FROM node:24-alpine AS runner
RUN apk add --no-cache font-dejavu
WORKDIR /app
ENV NODE_ENV=production
ENV HOST=0.0.0.0
ENV PORT=3000
COPY --from=runtime-deps /runtime/node_modules ./node_modules
COPY --from=builder /app/build ./build
COPY --from=builder /app/server.mjs ./server.mjs
COPY --from=builder /app/package.json ./package.json
EXPOSE 3000
CMD ["node", "server.mjs"]
