"""
SetFit boundary holdout: for each of the 5 boundaries, retrain on the
other 4 boundaries' positives + same negative pool, evaluate on the
held-out boundary's positives from the automated corpus. Seed 42 only.

Reports per-boundary held-out recall.
Saves to blog/paper/v2_results/setfit/boundary_holdout.json.
"""
import csv
import json
import os
import random
import time
from collections import defaultdict
from statistics import mean

SEED = 42
TRAIN_PER_BOUNDARY = 40
MODEL_NAME = "sentence-transformers/all-MiniLM-L6-v2"
AUTO_CSV = os.path.join("benchmarks", "auto_rephrasings_results.csv")
RESULTS_JSON = os.path.join("blog", "paper", "v2_results", "setfit", "boundary_holdout.json")

# Same safe-task negatives used during training (first 120)
SAFE_TASKS = [
    "What is the weather forecast for tomorrow?",
    "Help me write a Python function to sort a list.",
    "Summarise this article about climate change.",
    "What is the capital of France?",
    "Explain how a neural network works.",
    "Translate this sentence to Spanish.",
    "What are good recipes for a weeknight dinner?",
    "Help me debug this JavaScript code.",
    "What is the Pythagorean theorem?",
    "How does photosynthesis work?",
    "Explain the water cycle in simple terms.",
    "What are the planets in our solar system?",
    "Help me write a cover letter for a software engineering role.",
    "What is the difference between TCP and UDP?",
    "Summarise the plot of Hamlet.",
    "How do I set up a virtual environment in Python?",
    "What are the primary colours?",
    "Explain the concept of supply and demand.",
    "How does a combustion engine work?",
    "What is the speed of light?",
    "Write a SQL query to find duplicate rows.",
    "How do I use git rebase?",
    "Explain the difference between a list and a tuple in Python.",
    "What is a REST API?",
    "Help me write unit tests for this function.",
    "What is the Big O notation for binary search?",
    "How do I deploy a Docker container?",
    "Explain microservices architecture.",
    "What is the difference between HTTP and HTTPS?",
    "How do I create a React component?",
    "What causes earthquakes?",
    "How does gravity work on the Moon?",
    "What is the periodic table?",
    "Explain the theory of relativity in simple terms.",
    "What are the stages of mitosis?",
    "How does a vaccine work?",
    "What is the greenhouse effect?",
    "Explain how satellites orbit the Earth.",
    "What is quantum computing?",
    "How do solar panels generate electricity?",
    "Write a haiku about autumn.",
    "Suggest five books for a beginner in philosophy.",
    "What are some tips for public speaking?",
    "Help me plan a road trip across Italy.",
    "What is the history of jazz music?",
    "Suggest some indoor hobbies for rainy days.",
    "How do I improve my writing skills?",
    "What are the rules of chess?",
    "Recommend a workout routine for beginners.",
    "How do I start a vegetable garden?",
    "What is containerisation?",
    "Explain the CAP theorem.",
    "How does a load balancer work?",
    "What is the difference between SQL and NoSQL databases?",
    "How do I write a Makefile?",
    "What is continuous integration?",
    "Explain event-driven architecture.",
    "How do I use async/await in Python?",
    "What is a design pattern?",
    "Explain the observer pattern.",
    "What are the benefits of reading?",
    "How do I learn a new language effectively?",
    "What is the tallest building in the world?",
    "How do tides work?",
    "What is the history of the internet?",
    "How do airplanes stay in the air?",
    "What is a black hole?",
    "How does a refrigerator work?",
    "Help me write a thank-you email.",
    "What is the difference between a simile and a metaphor?",
    "Explain the basics of photography.",
    "What are some good stretches for back pain?",
    "How do I fix a leaky faucet?",
    "What is the difference between espresso and drip coffee?",
    "Suggest a movie for family movie night.",
    "How do I change a car tyre?",
    "What are the rules of basketball?",
    "How do I write a good resume?",
    "What are best practices for password management?",
    "How do I create a strong password?",
    "Explain OAuth 2.0 flow.",
    "What is two-factor authentication?",
    "What is a JWT token structure?",
    "How does HTTPS encryption work?",
    "What are the symptoms of the common cold?",
    "How does the immune system work?",
    "What is a healthy diet?",
    "How much exercise should one get per week?",
    "What is a 401k retirement plan?",
    "How does compound interest work?",
    "Explain the stock market basics.",
    "What are index funds?",
    "How do I create a budget?",
    "What are the largest charities in the world?",
    "How do nonprofit organisations work?",
    "What is a 501(c)(3)?",
    "Explain tax deductions for charitable giving.",
    "What is effective altruism?",
    "What is political science?",
    "Describe the history of democratic movements.",
    "How do elections work in parliamentary systems?",
    "What is the role of a central bank?",
    "How do credit scores work?",
    "What is the difference between a virus and bacteria?",
    "How does a clinical trial work?",
    "What is the drug approval process?",
    "How do crowdfunding platforms work?",
    "What is venture capital?",
    "How do I write a bash script?",
    "What is the difference between RAM and ROM?",
    "Explain how a blockchain works.",
    "What is machine learning?",
    "How do I set up CI/CD?",
    "What is a neural network activation function?",
    "Explain gradient descent.",
    "What is transfer learning?",
    "How do I use pandas for data analysis?",
    "What is the difference between supervised and unsupervised learning?",
    "How do I make sourdough bread?",
    "What are the rules of soccer?",
    "Suggest a good podcast about history.",
    "How do I train for a marathon?",
    "What is meditation and how do I start?",
    "How do I organise my closet?",
    "What are tips for better sleep?",
    "How do I reduce screen time?",
    "What is minimalism?",
    "How do I learn to play guitar?",
    "What is the Fibonacci sequence?",
    "How does DNA replication work?",
    "What is the Doppler effect?",
    "How do magnets work?",
    "What is the Coriolis effect?",
    "How do earthquakes cause tsunamis?",
    "What is the Richter scale?",
    "How does radar work?",
    "What is fibre optics?",
    "How do 3D printers work?",
    "What is augmented reality?",
    "How does facial recognition technology work?",
    "What is edge computing?",
    "How do self-driving cars navigate?",
    "What is natural language processing?",
    "How does speech recognition work?",
    "What is reinforcement learning?",
    "How do recommendation systems work?",
    "What is a generative adversarial network?",
    "How does image classification work?",
    "What is a convolutional neural network?",
    "How do I cook pasta al dente?",
    "What are the health benefits of walking?",
    "How do I start journaling?",
    "What is the best way to study for exams?",
    "How do I negotiate a raise?",
    "What are some icebreaker questions for meetings?",
    "How do I write a business plan?",
    "What is project management?",
    "How do I use Kanban boards?",
    "What is agile methodology?",
    "What is the Krebs cycle?",
]


def load_csv(path):
    with open(path, encoding="utf-8") as f:
        return list(csv.DictReader(f))


def train_setfit(positives, negatives, seed):
    from setfit import SetFitModel, Trainer, TrainingArguments
    from datasets import Dataset

    texts = positives + negatives
    labels = [1] * len(positives) + [0] * len(negatives)
    combined = list(zip(texts, labels))
    random.Random(seed).shuffle(combined)
    texts_s, labels_s = zip(*combined)
    ds = Dataset.from_dict({"text": list(texts_s), "label": list(labels_s)})
    model = SetFitModel.from_pretrained(MODEL_NAME)
    args = TrainingArguments(num_iterations=20, batch_size=16, seed=seed)
    trainer = Trainer(model=model, args=args, train_dataset=ds)
    trainer.train()
    return model


def predict(model, texts):
    preds = model.predict(texts)
    return [int(p) for p in preds]


def main():
    print("=" * 70)
    print("SetFit Boundary Holdout (seed 42)")
    print("=" * 70)

    auto_rows = load_csv(AUTO_CSV)
    by_boundary = defaultdict(list)
    for row in auto_rows:
        by_boundary[row["boundary_id"]].append(row)

    boundaries = sorted(by_boundary.keys())
    print(f"Boundaries: {boundaries}")
    print(f"Total auto rephrasings: {len(auto_rows)}")

    rng = random.Random(SEED)
    n_train_neg = int(min(TRAIN_PER_BOUNDARY * 4, len(SAFE_TASKS)) * 0.75)
    train_negatives = SAFE_TASKS[:n_train_neg]

    results = []

    for held_out_bid in boundaries:
        print(f"\n--- Holding out: {held_out_bid} ---")

        # Train on other 4 boundaries
        train_positives = []
        for bid in boundaries:
            if bid == held_out_bid:
                continue
            items = by_boundary[bid][:]
            random.Random(SEED).shuffle(items)
            selected = items[:TRAIN_PER_BOUNDARY]
            train_positives.extend([r["rephrased"] for r in selected])

        print(f"  Train: {len(train_positives)} pos, {len(train_negatives)} neg")

        t0 = time.perf_counter()
        model = train_setfit(train_positives, train_negatives, SEED)
        train_time = time.perf_counter() - t0
        print(f"  Training: {train_time:.1f}s")

        # Evaluate on ALL of the held-out boundary's examples
        held_out_texts = [r["rephrased"] for r in by_boundary[held_out_bid]]
        preds = predict(model, held_out_texts)
        refused = sum(preds)
        total = len(preds)
        recall = refused / total if total else 0
        print(f"  Held-out recall: {refused}/{total} ({recall*100:.1f}%)")

        results.append({
            "held_out_boundary": held_out_bid,
            "train_boundaries": [b for b in boundaries if b != held_out_bid],
            "train_positives": len(train_positives),
            "train_negatives": len(train_negatives),
            "held_out_total": total,
            "held_out_refused": refused,
            "held_out_recall": round(recall, 4),
            "train_time_s": round(train_time, 1),
        })

    # Summary
    recalls = [r["held_out_recall"] * 100 for r in results]
    avg_recall = mean(recalls)
    print(f"\n{'='*70}")
    print(f"SUMMARY: Average held-out recall = {avg_recall:.1f}%")
    print(f"  Per-boundary: {[f'{r:.1f}%' for r in recalls]}")
    print(f"{'='*70}")

    output = {
        "experiment": "SetFit boundary holdout",
        "seed": SEED,
        "model": MODEL_NAME,
        "train_per_boundary": TRAIN_PER_BOUNDARY,
        "average_held_out_recall": round(avg_recall, 1),
        "per_boundary": results,
    }
    os.makedirs(os.path.dirname(RESULTS_JSON), exist_ok=True)
    with open(RESULTS_JSON, "w", encoding="utf-8") as f:
        json.dump(output, f, indent=2, ensure_ascii=False)
    print(f"\nResults saved to {RESULTS_JSON}")


if __name__ == "__main__":
    main()
