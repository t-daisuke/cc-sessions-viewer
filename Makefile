.PHONY: release-patch release-minor release-beta

release-patch:
	@VERSION=$$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1); \
	MAJOR=$$(echo $$VERSION | cut -d. -f1); \
	MINOR=$$(echo $$VERSION | cut -d. -f2); \
	PATCH=$$(echo $$VERSION | cut -d. -f3); \
	NEW="$$MAJOR.$$MINOR.$$((PATCH + 1))"; \
	sed -i '' "s/^version = \".*\"/version = \"$$NEW\"/" Cargo.toml && \
	git add Cargo.toml && \
	git commit -m "$$NEW" && \
	git tag "v$$NEW" && \
	git push origin main && \
	git push origin "v$$NEW" && \
	echo "Released v$$NEW"

release-minor:
	@VERSION=$$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1); \
	MAJOR=$$(echo $$VERSION | cut -d. -f1); \
	MINOR=$$(echo $$VERSION | cut -d. -f2); \
	NEW="$$MAJOR.$$((MINOR + 1)).0"; \
	sed -i '' "s/^version = \".*\"/version = \"$$NEW\"/" Cargo.toml && \
	git add Cargo.toml && \
	git commit -m "$$NEW" && \
	git tag "v$$NEW" && \
	git push origin main && \
	git push origin "v$$NEW" && \
	echo "Released v$$NEW"

release-beta:
	@VERSION=$$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1); \
	MAJOR=$$(echo $$VERSION | cut -d. -f1); \
	MINOR=$$(echo $$VERSION | cut -d. -f2); \
	NEXT="$$MAJOR.$$((MINOR + 1)).0"; \
	N=$$(( $$(git tag -l "v$$NEXT-beta.*" | wc -l) + 1 )); \
	TAG="v$$NEXT-beta.$$N"; \
	git tag "$$TAG" && \
	git push origin "$$TAG" && \
	echo "Released $$TAG"
