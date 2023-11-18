import string
import evdev

def main():

    generated = []
    for c in string.ascii_lowercase:

        enum_name = f"KEY_{c.upper()}"
        keycode = evdev.ecodes.ecodes[enum_name]

        generated.append({
            "keycode": keycode,
            "keycode_line": f"    {enum_name} = {keycode},",
            "charmap_line": f"    (Keycode::{enum_name}, (Some('{c}'), Some('{c.upper()}'))),"
        })

    for c in string.digits:

        enum_name = f"KEY_{c}"
        keycode = evdev.ecodes.ecodes[enum_name]

        generated.append({
            "keycode": keycode,
            "keycode_line": f"    {enum_name} = {keycode},",
            "charmap_line": f"    (Keycode::{enum_name}, (Some('{c}'), Some('{c}'))),"
        })

    generated = sorted(generated, key=lambda it: it["keycode"])

    print("\n".join(it["keycode_line"] for it in generated))
    print("\n")
    print("\n".join(it["charmap_line"] for it in generated))

if __name__ == "__main__":
    main()
