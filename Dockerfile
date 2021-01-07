FROM registry.redhat.io/rhel8/rust-toolset as builder
COPY . .
RUN cargo build --release

FROM registry.fedoraproject.org/fedora:33
RUN yum -y install koji && yum clean all
COPY --from=builder /opt/app-root/src/target/release/* /usr/bin/
EXPOSE 8080
CMD ["/usr/bin/koji-sane-json-api"]
