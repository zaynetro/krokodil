.DEFAULT_GOAL := help

TIMESTAMP := $(shell date +%Y%m%d-%H%M%S )

.PHONY: help
# From: http://disq.us/p/16327nq
help: ## This help.
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / {printf "\033[36m%-30s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)

.PHONY: run
run:  ## Run server (hosts UI as well)
	cargo run


.PHONY: ui
ui:  ## Rebuild UI on file changes
	(cd ui && npm run watch)
