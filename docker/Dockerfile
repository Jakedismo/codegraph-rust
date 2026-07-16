FROM node:20-alpine

RUN apk add --no-cache git ripgrep jq bash

RUN npm install -g @colbymchenry/codegraph supergateway

WORKDIR /repo

ENV CODEGRAPH_STARTUP_HANDSHAKE_TIMEOUT_MS=0

EXPOSE 8045

ENTRYPOINT ["supergateway", "--port", "8045", "--stdio", "codegraph serve --mcp"]
