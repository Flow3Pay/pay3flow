// A narrow SDK entry also lets the dev optimizer discard provider UI exports.
export {
  WidgetConfigProvider, useTokenInputPair, useUnsafeSnapshot, useWalletConnection,
  useStoreSideEffects, useMakeTransfer, useSwitchChain, useOneClickExecutionStatus,
  fireEvent, isValidChainAddress,
} from "@aurora-is-near/intents-swap-widget-standalone";
export { useConfig } from "@aurora-is-near/intents-swap-widget/config";
