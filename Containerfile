# Build image: Fedora 44 matches the glibc and libSDL3 on the HTPC (Bazzite).
FROM registry.fedoraproject.org/fedora:44

RUN dnf install -y gcc SDL3-devel && dnf clean all

ENV RUSTUP_HOME=/opt/rustup CARGO_HOME=/opt/cargo PATH=/opt/cargo/bin:$PATH
RUN curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --no-modify-path \
    && chmod -R a+w /opt/cargo /opt/rustup

WORKDIR /src
