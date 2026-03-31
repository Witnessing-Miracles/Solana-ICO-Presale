# Use the official Node.js mirror
FROM node:18-alpine AS base

# 1. Install dependencies
FROM base AS deps
RUN apk add --no-cache libc6-compat
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm install --legacy-peer-deps

# 2. Build Project
FROM base AS builder
WORKDIR /app
COPY --from=deps /node_modules ./node_modules
COPY . .
RUN npm run build

# 3. Run image
FROM base AS runner
WORKDIR /app
ENV NODE_ENV production
COPY --from=builder /public ./public
COPY --from=builder /.next ./.next
COPY --from=builder /node_modules ./node_modules
COPY --from=builder /package.json ./package.json

EXPOSE 3000
CMD ["npm", "start"]
