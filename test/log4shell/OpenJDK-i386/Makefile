MAKEFILE_PATH := $(abspath $(lastword $(MAKEFILE_LIST)))
MAKEFILE_DIR := $(patsubst %/,%,$(dir $(MAKEFILE_PATH)))
IMAGE_NAME := trailofbits/openjdk-i386

.PHONY: docker
docker .last_built: Dockerfile
	docker build -t $(IMAGE_NAME) -f $(MAKEFILE_DIR)/Dockerfile $(MAKEFILE_DIR)
	@touch .last_built
	@echo Built trailofbits/openjdk-i386

.PHONY: rebuild
rebuild:
	docker build --no-cache -t $(IMAGE_NAME) -f $(MAKEFILE_DIR)/Dockerfile $(MAKEFILE_DIR)
	@touch .last_built
	@echo Built trailofbits/openjdk-i386

.PHONY: clean
clean:
	@rm -f .last_built
	-docker image rm $(IMAGE_NAME)
