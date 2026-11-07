"""Big-endian, typed NBT used by the Dragonfyre content and world fixtures."""
from dataclasses import dataclass
from io import BytesIO
from struct import calcsize, pack, unpack


@dataclass
class Tag:
    kind: int
    value: object


NUMBERS = {1: 'b', 2: 'h', 3: 'i', 4: 'q', 5: 'f', 6: 'd'}
ARRAYS = {7: 'b', 11: 'i', 12: 'q'}


def decode(data):
    stream = BytesIO(data)

    def number(fmt):
        return unpack('>' + fmt, stream.read(calcsize('>' + fmt)))[0]

    def string():
        # Preserve Java's modified UTF-8 bytes, including book text and emoji.
        return stream.read(number('H'))

    def tag(kind):
        if kind in NUMBERS:
            value = number(NUMBERS[kind])
        elif kind in ARRAYS:
            count = number('i')
            if kind == 7:
                value = stream.read(count)
            else:
                value = [number(ARRAYS[kind]) for _ in range(count)]
        elif kind == 8:
            value = string()
        elif kind == 9:
            member = number('B')
            value = member, [tag(member).value for _ in range(number('i'))]
        elif kind == 10:
            value = {}
            while member := number('B'):
                name = string()
                value[name] = tag(member)
        else:
            raise ValueError(f'Unknown NBT tag {kind}')
        return Tag(kind, value)

    kind = number('B')
    name = string()
    result = tag(kind)
    if stream.read(1):
        raise ValueError('Data follows NBT root')
    return name, result


def encode(name, root):
    stream = BytesIO()

    def number(fmt, value):
        stream.write(pack('>' + fmt, value))

    def string(value):
        number('H', len(value))
        stream.write(value)

    def tag(kind, value):
        if kind in NUMBERS:
            number(NUMBERS[kind], value)
        elif kind in ARRAYS:
            number('i', len(value))
            if kind == 7:
                stream.write(value)
            else:
                for member in value:
                    number(ARRAYS[kind], member)
        elif kind == 8:
            string(value)
        elif kind == 9:
            member, elements = value
            number('B', member)
            number('i', len(elements))
            for element in elements:
                tag(member, element)
        elif kind == 10:
            for key, item in value.items():
                number('B', item.kind)
                string(key)
                tag(item.kind, item.value)
            number('B', 0)
        else:
            raise ValueError(f'Unknown NBT tag {kind}')

    number('B', root.kind)
    string(name)
    tag(root.kind, root.value)
    return stream.getvalue()
