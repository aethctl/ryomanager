import {
  AppIcon,
  EndGroup,
  EndProcess,
  OpenLocation,
  ProcessDetail,
  SetPriority,
  SignalProcess,
  Snapshot,
} from "../../wailsjs/go/main/App";

export async function invoke<T>(command: string, args: Record<string, any> = {}): Promise<T> {
  switch (command) {
    case "snapshot":
      return (await Snapshot()) as T;
    case "app_icon":
      return (await AppIcon(args.key)) as T;
    case "process_detail":
      return (await ProcessDetail(args.pid, args.startTime)) as T;
    case "end_process":
      return (await EndProcess(args.pid, args.startTime, args.force)) as T;
    case "end_group":
      return (await EndGroup(args.key, args.force)) as T;
    case "signal_process":
      return (await SignalProcess(args.pid, args.startTime, args.action)) as T;
    case "set_priority":
      return (await SetPriority(args.pid, args.startTime, args.nice)) as T;
    case "open_location":
      return (await OpenLocation(args.pid, args.startTime)) as T;
    default:
      throw new Error(`unknown backend command: ${command}`);
  }
}
