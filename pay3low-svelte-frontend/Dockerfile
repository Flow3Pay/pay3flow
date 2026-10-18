FROM node:24-alpine AS deps
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci --ignore-scripts

FROM deps AS builder
WORKDIR /app
COPY . .
RUN NODE_OPTIONS=--max-old-space-size=6144 npm run build

FROM deps AS runner
RUN apk add --no-cache font-dejavu
WORKDIR /app
ENV NODE_ENV=production
ENV HOST=0.0.0.0
ENV PORT=3000
COPY --from=builder /app/build ./build
COPY --from=builder /app/server.mjs ./server.mjs
COPY --from=builder /app/package.json ./package.json
EXPOSE 3000
CMD ["node", "server.mjs"]
