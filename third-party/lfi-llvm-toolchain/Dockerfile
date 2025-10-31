FROM docker.io/library/ubuntu:24.04

ARG REPO_URL="https://github.com/lfi-project/lfi-llvm-toolchain"
ARG REPO_GIT_REV="65b66190707983d1b2fc110f07ffe6432fb3eeec"
ARG TARGET_ARCH="x86_64"

ENV DEBIAN_FRONTEND=noninteractive

ENV LFI_USER=root
ENV LFI_GROUP=root
ENV WORK_DIR=/root
ENV INSTALL_DIR=/opt/lfi
ENV TOOLCHAIN_BUILD_DIR=${INSTALL_DIR}/llvm-toolchain-build
ENV LFI_TOOLS_DIR=${INSTALL_DIR}/lfi-tools
ENV LFI_CLANG_DIR=${INSTALL_DIR}/${TARGET_ARCH}-lfi-clang
ENV NATIVE_CLANG_DIR=${INSTALL_DIR}/${TARGET_ARCH}-native-clang
ENV LFI_STORES_CLANG_DIR=${INSTALL_DIR}/${TARGET_ARCH}-lfi-stores-clang
ENV LFI_JUMPS_CLANG_DIR=${INSTALL_DIR}/${TARGET_ARCH}-lfi-jumps-clang
ENV CCACHE_DIR=/ccache

# Add the lfi-tools bin to the PATH, as that will be required for building the
# LFI toolchains later. We don't add the LFI toolchains yet, to avoid those
# compilers being picked up for later toolchain builds.
ENV PATH=${LFI_TOOLS_DIR}/bin:${PATH}

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    # Dependencies required to build LFI
    meson \
    ninja-build \
    build-essential \
    git \
    ccache \
    clang \
    lld \
    llvm \
    cmake \
    golang-go \
    rsync \
    wget \
    ca-certificates \
    # Dependencies useful for development
    vim \
    gdb \
    curl && \
    # Clean up apt cache to reduce image size
    rm -rf /var/lib/apt/lists/*

RUN if [ "${LFI_USER}" != "root" ]; then \
        groupadd -r ${LFI_USER} && \
        useradd -m -g ${LFI_GROUP} ${LFI_USER}; \
    fi && \
    mkdir -p ${WORK_DIR} ${INSTALL_DIR} ${LFI_TOOLS_DIR} ${LFI_CLANG_DIR} \
        ${NATIVE_CLANG_DIR} ${LFI_STORES_CLANG_DIR} ${LFI_JUMPS_CLANG_DIR} \
        ${CCACHE_DIR} && \
    chown ${LFI_USER}:${LFI_GROUP} -R ${WORK_DIR} ${INSTALL_DIR} ${LFI_TOOLS_DIR} \
        ${LFI_CLANG_DIR} ${NATIVE_CLANG_DIR} ${LFI_STORES_CLANG_DIR} \
        ${LFI_JUMPS_CLANG_DIR} ${CCACHE_DIR}
USER ${LFI_USER}

WORKDIR ${TOOLCHAIN_BUILD_DIR}

RUN git clone ${REPO_URL} . && \
    git checkout ${REPO_GIT_REV} && \
    ./download.sh

RUN --mount=type=cache,target=${CCACHE_DIR},id=lfi-ccache \
    ./build-lfi-tools.sh ${LFI_TOOLS_DIR} ${TARGET_ARCH}

# These are the main build steps and will take a significant amount of time.
# The PATH is already set, so lfi-rewrite and lfi-postlink are available.
RUN --mount=type=cache,target=${CCACHE_DIR},id=lfi-ccache \
    ./build-lfi.sh ${LFI_CLANG_DIR} ${TARGET_ARCH}
RUN --mount=type=cache,target=${CCACHE_DIR},id=lfi-ccache \
    LFISTORES=1 ./build-lfi.sh ${LFI_STORES_CLANG_DIR} ${TARGET_ARCH}
RUN --mount=type=cache,target=${CCACHE_DIR},id=lfi-ccache \
    LFIJUMPS=1 ./build-lfi.sh ${LFI_JUMPS_CLANG_DIR} ${TARGET_ARCH}
RUN --mount=type=cache,target=${CCACHE_DIR},id=lfi-ccache \
    ./build-native.sh ${NATIVE_CLANG_DIR} ${TARGET_ARCH}

ENV PATH=${LFI_TOOLS_DIR}/bin:${LFI_CLANG_DIR}/lfi-bin:${PATH}
WORKDIR ${WORK_DIR}

# This is a development container, we want it to run until it is explicitly
# terminated:
CMD ["/bin/sleep", "infinity"]
