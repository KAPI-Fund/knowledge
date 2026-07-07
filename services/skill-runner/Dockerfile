# skill-runner sidecar:跑在 KVM 主机上,和 CubeSandbox 同网。
FROM golang:1.25 AS build
WORKDIR /src
COPY go.mod go.sum ./
RUN go mod download
COPY . .
RUN CGO_ENABLED=0 go build -o /skill-runner ./...

FROM gcr.io/distroless/static-debian12
COPY --from=build /skill-runner /skill-runner
EXPOSE 4600
ENTRYPOINT ["/skill-runner"]
