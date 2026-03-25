#!/usr/bin/env python3
"""卡多宝 多号切换腳本 (uiautomator2 版)

用法:
    # 用號碼序號切換 (1-16)
    python3 switch_phone_number.py 03157df3c91de513 --index 5

    # 用號碼切換
    python3 switch_phone_number.py 03157df3c91de513 --number 05905349407

    # 列出所有號碼
    python3 switch_phone_number.py 03157df3c91de513 --list

    # 查看目前使用中的號碼
    python3 switch_phone_number.py 03157df3c91de513 --current
"""

import argparse
import sys
import time

import uiautomator2 as u2
import xml.etree.ElementTree as ET


def connect_device(device_id: str) -> u2.Device:
    d = u2.connect(device_id)
    print(f"已連接: {device_id}")
    return d


def open_stk(d: u2.Device):
    """強制關閉再重新打開 STK app"""
    d.app_stop("com.android.stk")
    time.sleep(1)
    d.shell("am start -n com.android.stk/.StkLauncherActivity")
    time.sleep(2)


def enter_switch_menu(d: u2.Device):
    """進入多号切换，如果已經在號碼列表就直接用"""
    # 已經在號碼列表裡了
    if d(text="SIM 工具包").exists(timeout=1) or d(textContains="9053").exists(timeout=1):
        return
    el = d(text="多号切换")
    if not el.exists(timeout=3):
        raise RuntimeError("找不到「多号切换」選單")
    el.click()
    time.sleep(2)


def collect_numbers(d: u2.Device) -> list[dict]:
    """滑動收集所有號碼，回傳 [{index, number, active}]"""
    # 先滑到頂部
    for _ in range(3):
        d.swipe_ext("down", scale=0.8)
        time.sleep(0.3)

    numbers = []
    seen = set()
    no_new_count = 0

    for _ in range(8):
        xml = d.dump_hierarchy()
        root = ET.fromstring(xml)
        found_new = False
        for node in root.iter("node"):
            text = node.get("text", "").strip()
            pkg = node.get("package", "")
            if not text or pkg != "com.android.stk":
                continue
            # 號碼格式: 數字開頭或 * 開頭，且包含數字
            clean = text.lstrip("*").strip()
            if not clean or not clean[0].isdigit():
                continue
            # 排除非號碼文字
            if clean in ("SIM 工具包",):
                continue
            active = text.startswith("*")
            if clean not in seen:
                seen.add(clean)
                numbers.append({
                    "index": len(numbers) + 1,
                    "number": clean,
                    "active": active,
                })
                found_new = True

        if not found_new:
            no_new_count += 1
            if no_new_count >= 2:
                break
        else:
            no_new_count = 0

        d.swipe_ext("up", scale=0.8)
        time.sleep(0.5)

    return numbers


def find_and_click_number(d: u2.Device, target: str):
    """滑動找到目標號碼並點擊"""
    # 先滑到頂部
    for _ in range(5):
        d.swipe_ext("down", scale=0.8)
        time.sleep(0.5)

    for _ in range(10):
        # 嘗試找 *號碼 或 號碼
        if d(textContains=target).exists(timeout=1):
            d(textContains=target).click()
            print(f"已點擊: {target}")
            return True
        d.swipe_ext("up", scale=0.8)
        time.sleep(0.8)

    return False


def get_current_number(numbers: list[dict]) -> str | None:
    for n in numbers:
        if n["active"]:
            return n["number"]
    return None


def do_list(d: u2.Device):
    open_stk(d)
    enter_switch_menu(d)
    numbers = collect_numbers(d)
    current = get_current_number(numbers)
    print(f"\n共 {len(numbers)} 個號碼 (目前使用: {current}):\n")
    for n in numbers:
        mark = " *" if n["active"] else "  "
        print(f"  {n['index']:>2}.{mark} {n['number']}")


def do_current(d: u2.Device):
    open_stk(d)
    enter_switch_menu(d)
    numbers = collect_numbers(d)
    current = get_current_number(numbers)
    if current:
        print(f"目前使用: {current}")
    else:
        print("無法判斷目前號碼")


def do_switch(d: u2.Device, target_number: str):
    open_stk(d)
    enter_switch_menu(d)

    # 先確認目前號碼
    numbers = collect_numbers(d)
    current = get_current_number(numbers)

    if current == target_number:
        print(f"目前已經是 {target_number}，不需要切換")
        return

    # 重新進入選單（因為 collect 會滑動）
    open_stk(d)
    enter_switch_menu(d)
    time.sleep(1)

    print(f"切換: {current} -> {target_number}")
    if find_and_click_number(d, target_number):
        time.sleep(2)
        print("切換完成（設備可能會自動重啟）")
    else:
        print(f"錯誤: 找不到號碼 {target_number}")
        sys.exit(1)


def main():
    parser = argparse.ArgumentParser(description="卡多宝 多号切换")
    parser.add_argument("device_id", help="ADB device ID")
    parser.add_argument("--index", type=int, help="用序號切換 (1-16)")
    parser.add_argument("--number", type=str, help="用號碼切換")
    parser.add_argument("--list", action="store_true", help="列出所有號碼")
    parser.add_argument("--current", action="store_true", help="查看目前號碼")
    args = parser.parse_args()

    d = connect_device(args.device_id)

    if args.list:
        do_list(d)
    elif args.current:
        do_current(d)
    elif args.index:
        # 先取得號碼列表對照 index
        open_stk(d)
        enter_switch_menu(d)
        numbers = collect_numbers(d)
        if args.index < 1 or args.index > len(numbers):
            print(f"錯誤: 序號必須在 1-{len(numbers)} 之間")
            sys.exit(1)
        target = numbers[args.index - 1]["number"]
        do_switch(d, target)
    elif args.number:
        do_switch(d, args.number)
    else:
        parser.print_help()


if __name__ == "__main__":
    main()
