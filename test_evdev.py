from evdev import InputDevice, categorize, ecodes

#dev = InputDevice('/dev/input/event11')
dev = InputDevice('/dev/input/event12')

print(dev)

for event in dev.read_loop():
    if event.type == ecodes.EV_KEY and event.value == 1:
        print(event)
        print(categorize(event))
