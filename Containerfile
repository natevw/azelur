# tested w/1.99.1-alpine3.24 initially
FROM docker.io/rust:alpine AS build
WORKDIR /usr/src/azelur
COPY . .
RUN cargo install --path .

FROM docker.io/alpine
COPY --from=build /usr/local/cargo/bin/azelur /usr/local/bin/azelur
ENTRYPOINT ["azelur", "0.0.0.0:4533"]
EXPOSE 4533/udp
