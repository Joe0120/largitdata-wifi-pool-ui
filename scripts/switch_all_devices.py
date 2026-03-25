#!/usr/bin/env python3
"""批次切換所有裝置的卡多宝號碼 (並行版)

用法:
    # 所有裝置同時切到第 5 個號碼
    python3 switch_all_devices.py 5

    # 查看所有裝置目前使用的號碼
    python3 switch_all_devices.py --current
"""

import argparse
import json
import os
import sys
import time
from concurrent.futures import ThreadPoolExecutor, as_completed

import uiautomator2 as u2
import xml.etree.ElementTree as ET

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
JSON_PATH = os.path.join(SCRIPT_DIR, "device_phones.json")
if not os.path.exists(JSON_PATH):
    JSON_PATH = os.path.join(SCRIPT_DIR, "..", "device_phones.json")


def load_devices():
    with open(JSON_PATH, encoding="utf-8") as f:
        data = json.load(f)
    # 只回傳有 phone_number 的裝置
    return [d for d in data if any(c["phone_number"] for c in d["card"])]


def open_stk(d):
    d.app_stop("com.android.stk")
    time.sleep(1)
    d.shell("am start -n com.android.stk/.StkLauncherActivity")
    time.sleep(2)


def enter_switch_menu(d):
    el = d(text="多号切换")
    if el.exists(timeout=3):
        el.click()
        time.sleep(2)


def collect_numbers(d):
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
            clean = text.lstrip("*").strip()
            if not clean or not clean[0].isdigit():
                continue
            if clean in ("SIM 工具包",):
                continue
            active = text.startswith("*")
            if clean not in seen:
                seen.add(clean)
                numbers.append({"index": len(numbers) + 1, "number": clean, "active": active})
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


def find_and_click_number(d, target):
    for _ in range(5):
        if d(textContains=target).exists(timeout=1):
            d(textContains=target).click()
            return True
        d.swipe_ext("up", scale=0.8)
        time.sleep(0.5)
    return False


def get_current_number(numbers):
    for n in numbers:
        if n["active"]:
            return n["number"]
    return None


def switch_device(device_id, app_lable, app_order, total_cards):
    """切換單台裝置，點擊後驗證是否真的切換成功"""
    try:
        d = u2.connect(device_id)
        d.implicitly_wait(3)

        open_stk(d)
        enter_switch_menu(d)
        time.sleep(1)

        target = app_lable
        if not find_and_click_number(d, target):
            return f"[FAIL] {device_id} | 找不到 {target}"

        # 等待切換生效
        time.sleep(3)

        # 驗證：重新開 STK 檢查目標號碼是否變成 active (有 * 前綴)
        open_stk(d)
        enter_switch_menu(d)
        numbers = collect_numbers(d)

        current = get_current_number(numbers)
        if current and target in current:
            return f"[OK] {device_id} | 切換成功 app_order={app_order} ({target}) 目前: {current}"
        else:
            return f"[FAIL] {device_id} | 點擊了 {target} 但目前是 {current or '未知'}"

    except Exception as e:
        return f"[ERROR] {device_id} | {e}"


def get_current_all(device_id):
    """查詢單台裝置目前號碼"""
    try:
        d = u2.connect(device_id)
        d.implicitly_wait(3)
        open_stk(d)
        enter_switch_menu(d)
        numbers = collect_numbers(d)
        current = get_current_number(numbers)
        return f"{device_id} | 目前: {current} (共 {len(numbers)} 個號碼)"
    except Exception as e:
        return f"{device_id} | ERROR: {e}"


def main():
    parser = argparse.ArgumentParser(description="批次切換所有裝置的卡多宝號碼")
    parser.add_argument("app_order", nargs="?", type=int, help="要切換到第幾個號碼 (app_order)")
    parser.add_argument("--current", action="store_true", help="查看所有裝置目前號碼")
    args = parser.parse_args()

    devices = load_devices()

    if args.current:
        print(f"查詢 {len(devices)} 台裝置...\n")
        with ThreadPoolExecutor(max_workers=len(devices)) as pool:
            futures = {pool.submit(get_current_all, d["device_id"]): d for d in devices}
            for future in as_completed(futures):
                print(future.result())
        return

    if not args.app_order:
        parser.print_help()
        return

    target_order = args.app_order
    print(f"目標: 所有裝置切換到 app_order={target_order}\n")

    # 建立每台裝置要切換的目標
    tasks = []
    for device in devices:
        cards = device["card"]
        total = len(cards)

        # 找到對應 app_order 的 card
        target_card = None
        for card in cards:
            if card["app_order"] == target_order:
                target_card = card
                break

        if not target_card:
            print(f"[SKIP] {device['device_id']} | 只有 {total} 個號碼，沒有 app_order={target_order}")
            continue

        if not target_card["phone_number"]:
            print(f"[SKIP] {device['device_id']} | app_order={target_order} 沒有 phone_number")
            continue

        tasks.append({
            "device_id": device["device_id"],
            "app_lable": target_card["app_lable"],
            "app_order": target_order,
            "total_cards": total,
        })

    if not tasks:
        print("沒有需要切換的裝置")
        return

    print(f"開始切換 {len(tasks)} 台裝置...\n")

    with ThreadPoolExecutor(max_workers=len(tasks)) as pool:
        futures = {
            pool.submit(
                switch_device,
                t["device_id"],
                t["app_lable"],
                t["app_order"],
                t["total_cards"],
            ): t
            for t in tasks
        }
        for future in as_completed(futures):
            print(future.result())

    print("\n完成")


if __name__ == "__main__":
    main()
