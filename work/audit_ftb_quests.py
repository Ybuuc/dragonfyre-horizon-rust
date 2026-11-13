"""Summarize original FTB Quests chapters."""
import argparse
from collections import Counter
import json
from pathlib import Path

from read_stage07_quests import chapters


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("chapters", type=Path)
    args = parser.parse_args()
    files = list(chapters(args.chapters))
    if not files:
        parser.error("No chapter SNBT files found")

    quests = {}
    tasks = Counter()
    rewards = Counter()
    chapter_sizes = {}
    for path, entries in files:
        chapter_sizes[path.stem] = len(entries)
        for quest in entries:
            quest_id = str(quest["id"])
            if quest_id in quests:
                parser.error(f"Duplicate quest ID: {quest_id}")
            quests[quest_id] = quest
            tasks.update(item["type"] for item in quest.get("tasks", []))
            rewards.update(item["type"] for item in quest.get("rewards", []))

    dependencies = Counter(
        str(dep) for quest in quests.values() for dep in quest.get("dependencies", [])
    )
    print(json.dumps({
        "chapters": chapter_sizes,
        "quest_count": len(quests),
        "task_types": dict(tasks.most_common()),
        "reward_types": dict(rewards.most_common()),
        "quests_with_dependencies": sum(bool(q.get("dependencies")) for q in quests.values()),
        "unresolved_dependency_ids": sorted(dependencies.keys() - quests.keys()),
    }, indent=2, ensure_ascii=False))


if __name__ == "__main__":
    main()
