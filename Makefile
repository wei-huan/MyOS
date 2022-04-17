TARGET := riscv64gc-unknown-none-elf
MODE := release
DIR := $(shell pwd)
PACK_IMG_DIR := $(DIR)/myfs-pack
FAT32_PACK_DIR := $(DIR)/fat32-pack
USER_DIR := $(DIR)/user
IMG_DIR := $(USER_DIR)/target/$(TARGET)/$(MODE)
OS_DIR := $(DIR)/kernel
FS_IMG := $(IMG_DIR)/fs.img

PLATFORM ?= qemu

build:
	@make build -C $(OS_DIR)

sdcard:
	@make sdcard -C $(OS_DIR)

fat32-img:
	@make fat32-img -C $(OS_DIR)

fat32-oscomp-img:
	@make fat32-oscomp-img -C $(OS_DIR)

user:
	@make build -C $(USER_DIR)

fs-img:
	@rm -f $(FS_IMG)
	@cd $(PACK_IMG_DIR) && cargo run --release -- -s $(USER_DIR)/src/bin/ -t $(IMG_DIR)/

run: fs-img
	@make run -C $(OS_DIR)

debug:
	@make debug -C $(OS_DIR)

gdb:
	@make gdb -C $(OS_DIR)

disasm:
	@make disasm -C $(OS_DIR)

.PHONY: build sdcard user fs-img run debug gdb disasm
