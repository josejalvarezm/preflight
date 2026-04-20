"""Quick sanity summary of the LLM06-extended corpus."""
import csv
from collections import Counter

path = "benchmarks/v2/external_boundaries/llm06/auto_rephrasings_llm06.csv"
rows = list(csv.DictReader(open(path, encoding="utf-8")))
print(f"Total rows: {len(rows)}")

bc = Counter(r["boundary_id"] for r in rows)
for bid, n in bc.most_common():
    tf = sum(1 for r in rows if r["boundary_id"] == bid and r["contains_trigger"] == "no")
    th = n - tf
    print(f"  {bid:<30} total={n:<5} trigger-free={tf:<5} trigger-containing={th}")

print("\n--- 6 LLM06 trigger-free samples ---")
llm06_tf = [r for r in rows
            if r["boundary_id"] == "sensitive-info-disclosure"
            and r["contains_trigger"] == "no"][:6]
for i, r in enumerate(llm06_tf, 1):
    text = r["rephrased"][:130].replace("\n", " ")
    print(f"  {i}. {text}")

print("\n--- 3 LLM06 trigger-containing samples ---")
llm06_t = [r for r in rows
           if r["boundary_id"] == "sensitive-info-disclosure"
           and r["contains_trigger"] == "yes"][:3]
for i, r in enumerate(llm06_t, 1):
    text = r["rephrased"][:130].replace("\n", " ")
    print(f"  {i}. {text}")
