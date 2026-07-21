FROM node:24-alpine
WORKDIR /app
COPY package*.json ./
RUN npm ci --omit=dev
COPY src ./src
COPY public ./public
ENV DATA_DIR=/data
EXPOSE 3000
CMD ["node", "src/server.js"]
