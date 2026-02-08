FROM scratch
ARG TARGETARCH
COPY binaries/${TARGETARCH}/slack-cli /slack-cli
ENTRYPOINT ["/slack-cli"]
