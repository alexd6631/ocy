publish-core:
    cd ocy-core && cargo publish

publish: publish-core
    cargo publish