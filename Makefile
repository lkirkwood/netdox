##
# Netdox
#
# @file
# @version 0.1

.PHONY: *

build:
	cargo build

deps:
	command -v cargo || curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

	@if command -v podman || command -v docker; then true; else \
	  echo "podman or docker must be installed"; exit 1; fi

runtime =
ifneq (, $(shell command -v podman))
	runtime = podman
else ifneq (, $(shell command -v docker))
	runtime = docker
endif

## Setting up redis ##
redis-container-name = "netdox-test-redis"

reset-redis = $(runtime) rm -f $(redis-container-name) 2>&1 > /dev/null && \
	$(runtime) run -p 9999:6379 --name $(redis-container-name) \
		docker.io/redis 2>&1 > /dev/null

redis-unready = [[ $$($(runtime) exec $(redis-container-name) redis-cli ping) != "PONG" ]]

wait-for-redis = while $(redis-unready) \
	do sleep 1; \
	done

init-redis:
	@echo "Initialising redis...";
	@$(reset-redis) &
	@$(wait-for-redis)
	@echo "Redis ready!";

######################

test: export NETDOX_TEST_REDIS_URL = redis://localhost:9999/0
test: init-redis
test: deps
	cargo test

coverage: export NETDOX_TEST_REDIS_URL = redis://localhost:9999/0
coverage: init-redis
coverage: deps
	cargo tarpaulin

## Setting up pageseeder ##
pageseeder-container-name = "netdox-test-pageseeder"

reset-pageseeder = $(runtime) rm -f -t 1 $(pageseeder-container-name) 2>&1 > /dev/null && \
	$(runtime) run -p 9998:8282 --name $(pageseeder-container-name) \
		registry-gitlab.allette.com.au/pageseeder/dev-container:6.2003 2>&1

init-pageseeder:
	@echo "Initialising pageseeder - this might take a couple of minutes...";

	@./test/setup-ps.sh $$(while IFS= read -r line; do \
		echo "$$line" | grep "secret: .* client:" && break; \
	done < <($(reset-pageseeder)))

	@echo "Pageseeder ready!";

###########################


integration: export NETDOX_TEST_REDIS_URL = redis://localhost:9999/0
integration: export NETDOX_SECRET = this_is_the_secret!?
integration: init-redis
integration: init-pageseeder
integration: deps
	cargo run config load test/config-generated.toml
	cargo test
	cargo test integration_tests::integration_publish -- --ignored --nocapture
	cargo run publish

# end
