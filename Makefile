TARGET := riscv64gc-unknown-none-elf
MODE := release
DIR := $(shell pwd)
PACK_IMG_DIR := $(DIR)/myfs-pack
FAT32_PACK_DIR := $(DIR)/fat32-pack
USER_DIR := $(DIR)/user
USER_TAR_DIR := $(USER_DIR)/target/$(TARGET)/$(MODE)
OSCOMP_DIR := $(DIR)/oscomp
OSCOMP_TAR_DIR := $(OSCOMP_DIR)/build/riscv64
OS_DIR := $(DIR)/kernel
FS_IMG := $(IMG_DIR)/fs.img

PLATFORM ?= qemu

build:
	@make build -C $(OS_DIR)

sdcard:
	@make sdcard -C $(OS_DIR)

fat32:
	@cd $(FAT32_PACK_DIR) && ./qemu_fat32.sh

user:
	@make build -C $(USER_DIR)

fs-img: user
	@cp $(USER_TAR_DIR)/initproc $(OSCOMP_TAR_DIR)/
	@cp $(USER_TAR_DIR)/user_shell $(OSCOMP_TAR_DIR)/
	@rm -f $(FS_IMG)
	@cd $(PACK_IMG_DIR) && cargo run --release -- -s $(OSCOMP_TAR_DIR)/ -t $(OSCOMP_TAR_DIR)/
	@cp ./fs.img $(USER_TAR_DIR)/
# @cd $(PACK_IMG_DIR) && cargo run --release -- -s $(USER_DIR)/src/bin/ -t $(USER_TAR_DIR)/

run: fs-img
	@make run -C $(OS_DIR)

debug:
	@make debug -C $(OS_DIR)

gdb:
	@make gdb -C $(OS_DIR)

disasm:
	@make disasm -C $(OS_DIR)

.PHONY: build sdcard user fs-img run debug gdb disasm
