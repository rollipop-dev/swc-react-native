import {
  useAnimatedStyle,
  useDerivedValue,
  useSharedValue,
  useAnimatedReaction,
  withTiming,
  withSpring,
  runOnUI,
} from 'react-native-reanimated';

const SPRING_CONFIG = { damping: 12, stiffness: 90 };

export function AnimatedBox() {
  const offset = useSharedValue(0);
  const scale = useSharedValue(1);
  const rotation = useSharedValue(0);

  const animatedStyle = useAnimatedStyle(() => {
    const translateX = withSpring(offset.value, SPRING_CONFIG);
    const s = withTiming(scale.value, { duration: 250 });
    return {
      transform: [
        { translateX },
        { scale: s },
        { rotateZ: `${rotation.value}deg` },
      ],
      opacity: scale.value,
    };
  });

  const computed = useDerivedValue(() => {
    'worklet';
    return offset.value * scale.value + Math.cos(rotation.value);
  });

  useAnimatedReaction(
    () => computed.value,
    (next, prev) => {
      if (prev !== null && next !== prev) {
        rotation.value = withSpring(next * 0.1);
      }
    },
  );

  function press() {
    'worklet';
    offset.value = offset.value > 0 ? 0 : 100;
    scale.value = withTiming(scale.value === 1 ? 1.5 : 1);
  }

  function release() {
    runOnUI(() => {
      'worklet';
      offset.value = withSpring(0);
    })();
  }

  return { animatedStyle, press, release };
}
