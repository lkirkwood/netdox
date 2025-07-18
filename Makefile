##
# Netdox
#
# @file
# @version 0.1

build:
	cargo build

deps:
	command -v cargo || curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

	if command -v podman || command -v docker || command -v redis-server; then true; else \
	  echo "Redis server, podman or docker must be installed"; exit 1; fi

runtime =
ifneq (, $(shell command -v podman))
	runtime = podman
else ifneq (, $(shell command -v docker))
	runtime = docker
endif

ifdef runtime
	redis-cmd = $(runtime) rm -f netdox-test-redis 2>&1 > /dev/null; \
		$(runtime) run -p 9999:6379 --name netdox-test-redis docker.io/redis
else
	redis-cmd = redis-server -p 9999 2>&1 > /dev/null
endif

test: export NETDOX_TEST_REDIS_URL = redis://localhost:9999/0
test: deps
	@$(redis-cmd) &
	@sleep 1
	cargo test

coverage: export NETDOX_TEST_REDIS_URL = redis://localhost:9999/0
coverage: deps
	@$(redis-cmd) &
	@sleep 1
	cargo tarpaulin

# end
