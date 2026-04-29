import { Gesture } from 'react-native-gesture-handler';
import {
  useAnimatedStyle,
  useSharedValue,
  withDecay,
  FadeIn,
  FadeOut,
  Layout,
} from 'react-native-reanimated';

export function buildPan() {
  const x = useSharedValue(0);
  const y = useSharedValue(0);
  const startX = useSharedValue(0);
  const startY = useSharedValue(0);

  const pan = Gesture.Pan()
    .onBegin(() => {
      'worklet';
      startX.value = x.value;
      startY.value = y.value;
    })
    .onUpdate((event) => {
      'worklet';
      x.value = startX.value + event.translationX;
      y.value = startY.value + event.translationY;
    })
    .onEnd((event) => {
      'worklet';
      x.value = withDecay({ velocity: event.velocityX, clamp: [-200, 200] });
      y.value = withDecay({ velocity: event.velocityY, clamp: [-200, 200] });
    });

  const style = useAnimatedStyle(() => ({
    transform: [{ translateX: x.value }, { translateY: y.value }],
  }));

  return { pan, style };
}

export const enter = FadeIn.duration(220).withCallback((finished) => {
  'worklet';
  if (finished) {
    console.log('enter done');
  }
});

export const exit = FadeOut.duration(180);
export const layout = Layout.springify().damping(14);
