"""Read FTB Quests chapter SNBT, including newline-separated entries."""
import json
import re
from pathlib import Path

SOURCE = Path(__file__).resolve().parents[4] / (
    "referenced-chatgpt-conversation-this-is-an/outputs/Horizon-Forge/data/game/"
    "instances/cisco-s-dragonfyre-3c-beta-java-forge-local-import/config/ftbquests/quests/chapters"
)
TOKEN = re.compile(r'\s+|//[^\n]*|"(?:\\.|[^"\\])*"|[{}\[\]:;,]|[^\s{}\[\]:;,"]+')
NUMBER = re.compile(r'([+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:e[+-]?\d+)?)([bslfd]?)', re.I)


def parse(text):
    parts = [m[0] for m in TOKEN.finditer(text)]
    if ''.join(parts) != text:
        raise ValueError('invalid or unterminated token')
    tokens = [t for t in parts if not t.isspace() and not t.startswith('//')]
    i = 0

    def peek():
        return tokens[i] if i < len(tokens) else None

    def pop(expected=None):
        nonlocal i
        t = peek()
        if t is None or (expected is not None and t != expected):
            raise ValueError(f'token {i}: expected {expected}, got {t}')
        i += 1
        return t

    def key(t):
        if t.startswith('"'):
            return json.loads(t)
        if t in '{}[]:;,':
            raise ValueError(f'token {i}: invalid key {t}')
        return t

    def value():
        t = pop()
        if t == '{':
            fields = {}
            while peek() != '}':
                k = key(pop())
                if k in fields:
                    raise ValueError(f'duplicate key {k}')
                pop(':')
                fields[k] = value()
                if peek() == ',':
                    pop()
            pop('}')
            return fields
        if t == '[':
            if peek() in ('I', 'B', 'L') and tokens[i + 1:i + 2] == [';']:
                pop()
                pop(';')
            elements = []
            while peek() != ']':
                elements.append(value())
                if peek() == ',':
                    pop()
            pop(']')
            return elements
        if t in ('true', 'false'):
            return t == 'true'
        if m := NUMBER.fullmatch(t):
            n, suffix = m.groups()
            return float(n) if suffix.lower() in ('f', 'd') or any(c in n.lower() for c in '.e') else int(n)
        return key(t)

    result = value()
    if peek() is not None:
        raise ValueError(f'trailing token {peek()}')
    return result


def chapters(source=SOURCE):
    for path in sorted(source.glob('*.snbt')):
        try:
            yield path, parse(path.read_text(encoding='utf-8-sig'))['quests']
        except (ValueError, KeyError) as e:
            raise ValueError(f'{path}: {e}') from e
