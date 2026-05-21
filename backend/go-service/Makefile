BIN_DIR := bin
STAGE_DIR := ../apps/desktop/src-tauri/resources/bin

.PHONY: build build-stage clean

build:
	mkdir -p $(BIN_DIR)
	go build -o $(BIN_DIR)/pdfsvc ./cmd/pdfsvc

build-stage: build
	mkdir -p $(STAGE_DIR)
	cp $(BIN_DIR)/pdfsvc $(STAGE_DIR)/

clean:
	rm -rf $(BIN_DIR)
	rm -rf $(STAGE_DIR)/pdfsvc

