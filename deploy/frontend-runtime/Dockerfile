# Keep these deps/builder stages aligned with the frontend Dockerfile so an
# existing application build can be reused when only runtime packaging changes.
FROM node:24-alpine AS base
RUN corepack enable && corepack prepare pnpm@12.3.4 --activate

FROM base AS deps
WORKDIR /app
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
RUN --mount=type=cache,target=/root/.local/share/pnpm/store \
    pnpm install --frozen-lockfile --ignore-scripts

FROM deps AS builder
WORKDIR /app
COPY . .
RUN NODE_OPTIONS=--max-old-space-size=6144 pnpm run build

# Pack production dependencies into one archive so exporting the image does not
# need to walk and serialize tens of thousands of individual files.
FROM base AS runner
RUN apk add --no-cache font-dejavu
WORKDIR /app
ENV NODE_ENV=production
ENV HOST=0.0.0.0
ENV PORT=3000
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
RUN --mount=type=cache,target=/root/.local/share/pnpm/store \
    pnpm install --frozen-lockfile --prod --ignore-scripts \
    && find node_modules -type f \( -name '*.d.ts' -o -name '*.d.mts' -o -name '*.d.cts' \
         -o -name '*.map' -o -name '*.tsbuildinfo' \) -delete \
    && tar -czf node_modules.tar.gz node_modules \
    && rm -rf node_modules
COPY --from=builder /app/build ./build
COPY --from=builder /app/server.mjs ./server.mjs
EXPOSE 3000
CMD ["sh", "-c", "tar -xzf node_modules.tar.gz && exec node server.mjs"]
