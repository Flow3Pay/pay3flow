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

# Install the production dependencies directly in the final layer. Copying a
# filtered node_modules from another stage rewrote tens of thousands of small
# files into a fresh overlay layer and took half an hour on the production disk.
# The same layer drops files the Node runtime never reads: TypeScript
# declarations, source maps, build info, and the raw Material Symbols SVG font
# sources (the compiled React icons remain).
FROM node:24-alpine AS runner
RUN apk add --no-cache font-dejavu
WORKDIR /app
ENV NODE_ENV=production
ENV HOST=0.0.0.0
ENV PORT=3000
COPY package.json package-lock.json ./
RUN --mount=type=cache,target=/root/.npm \
    npm ci --omit=dev --ignore-scripts \
    && find node_modules -type f \( -name '*.d.ts' -o -name '*.d.mts' -o -name '*.d.cts' \
         -o -name '*.map' -o -name '*.tsbuildinfo' \) -delete \
    && find node_modules -type d -path '*/@material-symbols/svg-*' -prune -exec rm -rf {} +
COPY --from=builder /app/build ./build
COPY --from=builder /app/server.mjs ./server.mjs
EXPOSE 3000
CMD ["node", "server.mjs"]
