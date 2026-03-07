VERSION := $(shell grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')
MAJOR := $(shell echo $(VERSION) | cut -d. -f1)
MINOR := $(shell echo $(VERSION) | cut -d. -f2)
PATCH := $(shell echo $(VERSION) | cut -d. -f3)

.PHONY: patch minor major install current

current:
	@echo "smc v$(VERSION)"

patch:
	@NEW_V="$(MAJOR).$(MINOR).$(shell echo $$(($(PATCH)+1)))"; \
	sed -i '' "s/^version = \"$(VERSION)\"/version = \"$$NEW_V\"/" Cargo.toml; \
	echo "Bumped $(VERSION) → $$NEW_V"; \
	cargo install --path . 2>&1 | tail -1; \
	echo "smc $$NEW_V installed"

minor:
	@NEW_V="$(MAJOR).$(shell echo $$(($(MINOR)+1))).0"; \
	sed -i '' "s/^version = \"$(VERSION)\"/version = \"$$NEW_V\"/" Cargo.toml; \
	echo "Bumped $(VERSION) → $$NEW_V"; \
	cargo install --path . 2>&1 | tail -1; \
	echo "smc $$NEW_V installed"

major:
	@NEW_V="$(shell echo $$(($(MAJOR)+1))).0.0"; \
	sed -i '' "s/^version = \"$(VERSION)\"/version = \"$$NEW_V\"/" Cargo.toml; \
	echo "Bumped $(VERSION) → $$NEW_V"; \
	cargo install --path . 2>&1 | tail -1; \
	echo "smc $$NEW_V installed"

install:
	cargo install --path .
