"""
Phase 5 -- Safe-corpus false-positive rate evaluation.

Draws 500 instruction-type tasks from Dolly 15k (50--300 chars, English),
runs the AI-OS deterministic keyword engine on them, and reports the
false-positive rate (how many safe tasks are incorrectly refused).

Usage:
    python benchmarks/v2/safe_corpus/safe_fpr.py
"""

import csv
import json
import os
import re
import unicodedata
from collections import defaultdict

RESULTS_DIR = os.path.join("paper", "v2_results", "safe_corpus")
RESULTS_JSON = os.path.join(RESULTS_DIR, "results.json")
CORPUS_CSV = os.path.join("benchmarks", "v2", "safe_corpus", "dolly_500.csv")

# Same 5 boundaries as evasion_study.py / benchmark_enforcement.rs
BOUNDARIES = {
    "BOUNDARY-001": {
        "triggers": ["charity", "donation", "donate", "align", "patterns",
                      "political", "affiliation"],
        "subjects": ["political", "party", "voting", "donation"],
    },
    "BOUNDARY-002": {
        "triggers": ["password", "credential", "token", "secret", "key",
                      "api", "auth"],
        "subjects": ["password", "credential", "secret", "token"],
    },
    "BOUNDARY-003": {
        "triggers": ["medical", "health", "diagnosis", "prescription",
                      "condition", "treatment"],
        "subjects": ["medical", "health", "diagnosis", "prescription"],
    },
    "BOUNDARY-004": {
        "triggers": ["salary", "compensation", "income", "pay", "bonus",
                      "stock"],
        "subjects": ["salary", "income", "compensation"],
    },
    "BOUNDARY-005": {
        "triggers": ["location", "address", "home", "gps", "coordinates",
                      "whereabouts"],
        "subjects": ["location", "address", "home"],
    },
    "BOUNDARY-006": {
        "triggers": ["proprietary", "confidential", "internal", "source",
                      "algorithm", "model", "weights", "training", "dataset",
                      "pii", "personal"],
        "subjects": ["proprietary", "confidential", "internal", "source",
                      "algorithm", "weights", "training", "dataset", "pii",
                      "personal", "identifiable"],
    },
}


def normalize(word):
    return unicodedata.normalize("NFKC", word).lower()


def extract_keywords(text):
    words = re.findall(r'[a-zA-Z]+', normalize(text))
    return set(words)


def check_trigger_and_subject(keywords):
    for bid, b in BOUNDARIES.items():
        triggers = {normalize(t) for t in b["triggers"]}
        subjects = {normalize(s) for s in b["subjects"]}
        if (keywords & triggers) and (keywords & subjects):
            return True, bid, keywords & triggers, keywords & subjects
    return False, None, set(), set()


def load_dolly_corpus():
    """Load 500 instruction tasks from Dolly 15k via HuggingFace datasets."""
    if os.path.exists(CORPUS_CSV):
        print(f"Loading cached corpus from {CORPUS_CSV}")
        tasks = []
        with open(CORPUS_CSV, encoding="utf-8") as f:
            for row in csv.DictReader(f):
                tasks.append(row["instruction"])
        return tasks

    print("Downloading Dolly 15k from HuggingFace...")
    from datasets import load_dataset
    ds = load_dataset("databricks/databricks-dolly-15k", split="train")

    # Filter: instruction-type tasks, 50-300 chars, English
    # Categories that are instruction-like
    instruction_cats = {
        "open_qa", "closed_qa", "general_qa",
        "brainstorming", "creative_writing", "summarization",
        "information_extraction", "classification",
    }

    candidates = []
    for row in ds:
        inst = row["instruction"].strip()
        cat = row.get("category", "")
        if cat in instruction_cats and 50 <= len(inst) <= 300:
            candidates.append(inst)

    # Shuffle deterministically and take 500
    import hashlib
    candidates.sort(key=lambda x: hashlib.sha256(x.encode()).hexdigest())
    tasks = candidates[:500]

    # Cache to CSV
    os.makedirs(os.path.dirname(CORPUS_CSV), exist_ok=True)
    with open(CORPUS_CSV, "w", encoding="utf-8", newline="") as f:
        w = csv.DictWriter(f, fieldnames=["instruction"])
        w.writeheader()
        for t in tasks:
            w.writerow({"instruction": t})
    print(f"Cached {len(tasks)} tasks to {CORPUS_CSV}")

    return tasks


def run():
    print("=" * 70)
    print("SAFE-CORPUS FALSE-POSITIVE RATE (PHASE 5)")
    print("=" * 70)

    tasks = load_dolly_corpus()
    print(f"Safe corpus size: {len(tasks)} tasks")

    refused = []
    allowed = 0

    for task in tasks:
        kw = extract_keywords(task)
        fired, bid, t_match, s_match = check_trigger_and_subject(kw)
        if fired:
            refused.append({
                "task": task,
                "boundary": bid,
                "trigger_match": sorted(t_match),
                "subject_match": sorted(s_match),
            })
        else:
            allowed += 1

    fpr = len(refused) / len(tasks) * 100

    print(f"\nAI-OS Deterministic (trigger AND subject):")
    print(f"  Refused: {len(refused)}/{len(tasks)} ({fpr:.1f}%)")
    print(f"  Allowed: {allowed}/{len(tasks)} ({100-fpr:.1f}%)")

    if refused:
        print(f"\nFalse positive details:")
        for i, r in enumerate(refused, 1):
            print(f"  {i}. [{r['boundary']}] triggers={r['trigger_match']}, "
                  f"subjects={r['subject_match']}")
            print(f"     Task: {r['task'][:100]}...")

    results = {
        "corpus_size": len(tasks),
        "corpus_source": "databricks/databricks-dolly-15k",
        "filter": "instruction categories, 50-300 chars",
        "system": "AI-OS deterministic (trigger AND subject)",
        "refused": len(refused),
        "allowed": allowed,
        "fpr_pct": round(fpr, 1),
        "false_positives": refused,
    }

    os.makedirs(RESULTS_DIR, exist_ok=True)
    with open(RESULTS_JSON, "w", encoding="utf-8") as f:
        json.dump(results, f, indent=2)
    print(f"\nResults written to {RESULTS_JSON}")


if __name__ == "__main__":
    run()
