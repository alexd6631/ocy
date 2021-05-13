publish-all:
    cd ocy-core && cargo publish
    sleep 30
    cargo publish
