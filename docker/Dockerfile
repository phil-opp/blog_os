FROM rustlang/rust:nightly

ENV IMAGE_NAME=blog_os-docker

RUN apt-get update && \
    apt-get install -q -y --no-install-recommends \
    nasm \
    binutils \
    grub-common \
    xorriso \
    grub-pc-bin && \
    apt-get autoremove -q -y && \
    apt-get clean -q -y && \
    rm -rf /var/lib/apt/lists/* && \
    cargo install xargo && \
    rustup component add rust-src

ENV GOSU_VERSION 1.10

RUN set -ex; \
	\
	fetchDeps=' \
		ca-certificates \
		wget \
	'; \
	apt-get update; \
	apt-get install -y --no-install-recommends $fetchDeps; \
	rm -rf /var/lib/apt/lists/*; \
	\
	dpkgArch="$(dpkg --print-architecture | awk -F- '{ print $NF }')"; \
	wget -O /usr/local/bin/gosu "https://github.com/tianon/gosu/releases/download/$GOSU_VERSION/gosu-$dpkgArch"; \
	chmod +x /usr/local/bin/gosu; \
# verify that the binary works
	gosu nobody true; 

COPY entrypoint.sh /usr/local/bin/
COPY .bash_aliases /etc/skel/

ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
CMD ["/bin/bash"]
