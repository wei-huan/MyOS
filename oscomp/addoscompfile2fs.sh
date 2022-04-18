FAT32_DIR="../fat32-pack"
OSCOMP_TEST_DIR="./target/riscv64"
SELF_TEST_DIR="target/riscv64gc-unknown-none-elf/release"

FAT32_IMG="${FAT32_DIR}/fat32.img"
# FAT32_IMG="/dev/sda"

# sudo chmod -R 777 ${FAT32_IMG}
# sudo umount ${FAT32_IMG}
# mkfs.vfat -F 32 ${FAT32_IMG}

# 如果文件夹存在
if test -e ${FAT32_DIR}/fs
then
    sudo rm -r ${FAT32_DIR}/fs
    mkdir ${FAT32_DIR}/fs
else
    mkdir ${FAT32_DIR}/fs
fi

sudo mount ${FAT32_IMG} ${FAT32_DIR}/fs
sudo rm -rf ${FAT32_DIR}/fs/*

for inode in $(ls build/riscv64)
do
    sudo cp -r ./build/riscv64/${inode} ${FAT32_DIR}/fs/${inode}
done

for programname in $(ls ../user/src/bin)
do
    if [ $programname == "initproc.rs" ] || [ $programname == "user_shell.rs" ]
    then
    sudo cp ../user/target/riscv64gc-unknown-none-elf/release/${programname%.rs} ${FAT32_DIR}/fs/${programname%.rs}
    fi
done

sudo umount ${FAT32_DIR}/fs
