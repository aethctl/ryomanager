export namespace main {

	export class CpuCache {
	    level: string;
	    size: string;

	    static createFrom(source: any = {}) {
	        return new CpuCache(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.level = source["level"];
	        this.size = source["size"];
	    }
	}
	export class CpuCore {
	    index: number;
	    usage: number;
	    freqMhz?: number;
	    kind?: string;

	    static createFrom(source: any = {}) {
	        return new CpuCore(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.index = source["index"];
	        this.usage = source["usage"];
	        this.freqMhz = source["freqMhz"];
	        this.kind = source["kind"];
	    }
	}
	export class Pressure {
	    some10: number;
	    some60: number;
	    some300: number;
	    full10?: number;
	    full60?: number;
	    full300?: number;

	    static createFrom(source: any = {}) {
	        return new Pressure(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.some10 = source["some10"];
	        this.some60 = source["some60"];
	        this.some300 = source["some300"];
	        this.full10 = source["full10"];
	        this.full60 = source["full60"];
	        this.full300 = source["full300"];
	    }
	}
	export class CpuInfo {
	    usage: number;
	    user: number;
	    system: number;
	    iowait: number;
	    irq: number;
	    cores: CpuCore[];
	    freqMhz?: number;
	    maxFreqMhz?: number;
	    governor?: string;
	    loadAvg: number[];
	    uptimeSeconds: number;
	    processes: number;
	    threads: number;
	    openFiles?: number;
	    openFilesMax?: number;
	    model: string;
	    sockets: number;
	    physicalCores: number;
	    logicalCpus: number;
	    virtualization?: string;
	    caches: CpuCache[];
	    pressure?: Pressure;

	    static createFrom(source: any = {}) {
	        return new CpuInfo(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.usage = source["usage"];
	        this.user = source["user"];
	        this.system = source["system"];
	        this.iowait = source["iowait"];
	        this.irq = source["irq"];
	        this.cores = this.convertValues(source["cores"], CpuCore);
	        this.freqMhz = source["freqMhz"];
	        this.maxFreqMhz = source["maxFreqMhz"];
	        this.governor = source["governor"];
	        this.loadAvg = source["loadAvg"];
	        this.uptimeSeconds = source["uptimeSeconds"];
	        this.processes = source["processes"];
	        this.threads = source["threads"];
	        this.openFiles = source["openFiles"];
	        this.openFilesMax = source["openFilesMax"];
	        this.model = source["model"];
	        this.sockets = source["sockets"];
	        this.physicalCores = source["physicalCores"];
	        this.logicalCpus = source["logicalCpus"];
	        this.virtualization = source["virtualization"];
	        this.caches = this.convertValues(source["caches"], CpuCache);
	        this.pressure = this.convertValues(source["pressure"], Pressure);
	    }

		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}
	export class DiskMount {
	    path: string;
	    fs: string;
	    used: number;
	    total: number;
	    system: boolean;

	    static createFrom(source: any = {}) {
	        return new DiskMount(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.path = source["path"];
	        this.fs = source["fs"];
	        this.used = source["used"];
	        this.total = source["total"];
	        this.system = source["system"];
	    }
	}
	export class DiskInfo {
	    name: string;
	    model?: string;
	    kind: string;
	    capacity: number;
	    readRate: number;
	    writeRate: number;
	    readIops: number;
	    writeIops: number;
	    activePercent: number;
	    responseMs?: number;
	    readTotal: number;
	    writeTotal: number;
	    mounts: DiskMount[];

	    static createFrom(source: any = {}) {
	        return new DiskInfo(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.name = source["name"];
	        this.model = source["model"];
	        this.kind = source["kind"];
	        this.capacity = source["capacity"];
	        this.readRate = source["readRate"];
	        this.writeRate = source["writeRate"];
	        this.readIops = source["readIops"];
	        this.writeIops = source["writeIops"];
	        this.activePercent = source["activePercent"];
	        this.responseMs = source["responseMs"];
	        this.readTotal = source["readTotal"];
	        this.writeTotal = source["writeTotal"];
	        this.mounts = this.convertValues(source["mounts"], DiskMount);
	    }

		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}

	export class EnergyInfo {
	    source: string;
	    batteryPercent?: number;
	    batteryStatus?: string;
	    batteryPower?: number;
	    batteryEnergyNow?: number;
	    batteryEnergyFull?: number;
	    batteryEnergyDesign?: number;
	    timeToEmptySeconds?: number;
	    cycleCount?: number;
	    packagePower?: number;
	    limits: Record<string, string>;

	    static createFrom(source: any = {}) {
	        return new EnergyInfo(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.source = source["source"];
	        this.batteryPercent = source["batteryPercent"];
	        this.batteryStatus = source["batteryStatus"];
	        this.batteryPower = source["batteryPower"];
	        this.batteryEnergyNow = source["batteryEnergyNow"];
	        this.batteryEnergyFull = source["batteryEnergyFull"];
	        this.batteryEnergyDesign = source["batteryEnergyDesign"];
	        this.timeToEmptySeconds = source["timeToEmptySeconds"];
	        this.cycleCount = source["cycleCount"];
	        this.packagePower = source["packagePower"];
	        this.limits = source["limits"];
	    }
	}
	export class GpuInfo {
	    index: number;
	    name: string;
	    vendor: string;
	    usage?: number;
	    memoryUsage?: number;
	    encoder?: number;
	    decoder?: number;
	    memoryUsed?: number;
	    memoryTotal?: number;
	    temperature?: number;
	    power?: number;
	    powerLimit?: number;
	    clockMhz?: number;
	    memoryClockMhz?: number;
	    driver?: string;
	    pstate?: string;
	    limits: Record<string, string>;

	    static createFrom(source: any = {}) {
	        return new GpuInfo(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.index = source["index"];
	        this.name = source["name"];
	        this.vendor = source["vendor"];
	        this.usage = source["usage"];
	        this.memoryUsage = source["memoryUsage"];
	        this.encoder = source["encoder"];
	        this.decoder = source["decoder"];
	        this.memoryUsed = source["memoryUsed"];
	        this.memoryTotal = source["memoryTotal"];
	        this.temperature = source["temperature"];
	        this.power = source["power"];
	        this.powerLimit = source["powerLimit"];
	        this.clockMhz = source["clockMhz"];
	        this.memoryClockMhz = source["memoryClockMhz"];
	        this.driver = source["driver"];
	        this.pstate = source["pstate"];
	        this.limits = source["limits"];
	    }
	}
	export class MemoryInfo {
	    total: number;
	    used: number;
	    available: number;
	    free: number;
	    buffers: number;
	    cached: number;
	    shared: number;
	    dirty: number;
	    mapped: number;
	    committed: number;
	    commitLimit: number;
	    swapTotal: number;
	    swapUsed: number;
	    swapCached: number;
	    zswap?: number;
	    pressure?: Pressure;
	    limits: Record<string, string>;

	    static createFrom(source: any = {}) {
	        return new MemoryInfo(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.total = source["total"];
	        this.used = source["used"];
	        this.available = source["available"];
	        this.free = source["free"];
	        this.buffers = source["buffers"];
	        this.cached = source["cached"];
	        this.shared = source["shared"];
	        this.dirty = source["dirty"];
	        this.mapped = source["mapped"];
	        this.committed = source["committed"];
	        this.commitLimit = source["commitLimit"];
	        this.swapTotal = source["swapTotal"];
	        this.swapUsed = source["swapUsed"];
	        this.swapCached = source["swapCached"];
	        this.zswap = source["zswap"];
	        this.pressure = this.convertValues(source["pressure"], Pressure);
	        this.limits = source["limits"];
	    }

		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}
	export class NetworkInfo {
	    name: string;
	    kind: string;
	    state: string;
	    rxRate: number;
	    txRate: number;
	    rxTotal: number;
	    txTotal: number;
	    ipv4: string[];
	    ipv6: string[];
	    mac?: string;
	    mtu?: number;
	    speedMbps?: number;
	    ssid?: string;
	    signal?: number;
	    frequencyMhz?: number;
	    driver?: string;

	    static createFrom(source: any = {}) {
	        return new NetworkInfo(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.name = source["name"];
	        this.kind = source["kind"];
	        this.state = source["state"];
	        this.rxRate = source["rxRate"];
	        this.txRate = source["txRate"];
	        this.rxTotal = source["rxTotal"];
	        this.txTotal = source["txTotal"];
	        this.ipv4 = source["ipv4"];
	        this.ipv6 = source["ipv6"];
	        this.mac = source["mac"];
	        this.mtu = source["mtu"];
	        this.speedMbps = source["speedMbps"];
	        this.ssid = source["ssid"];
	        this.signal = source["signal"];
	        this.frequencyMhz = source["frequencyMhz"];
	        this.driver = source["driver"];
	    }
	}

	export class ProcessInfo {
	    pid: number;
	    parentPid?: number;
	    startTime: number;
	    name: string;
	    exe?: string;
	    command: string;
	    cwd?: string;
	    user: string;
	    uid: number;
	    state: string;
	    kernelThread: boolean;
	    unit?: string;
	    cpu: number;
	    cpuUser: number;
	    cpuSystem: number;
	    memory: number;
	    memoryKind: string;
	    rss: number;
	    rssAnon?: number;
	    rssFile?: number;
	    rssShmem?: number;
	    swap?: number;
	    virtual: number;
	    diskRead?: number;
	    diskWrite?: number;
	    diskReadTotal?: number;
	    diskWriteTotal?: number;
	    gpu?: number;
	    gpuMemory?: number;
	    gpuIndex?: number;
	    connections?: number;
	    energy: string;
	    energyScore: number;
	    threads: number;
	    fds?: number;
	    nice: number;
	    priority: number;
	    ctxSwitches?: number;
	    oomScore?: number;
	    lastCpu?: number;
	    windows: WindowRef[];

	    static createFrom(source: any = {}) {
	        return new ProcessInfo(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.pid = source["pid"];
	        this.parentPid = source["parentPid"];
	        this.startTime = source["startTime"];
	        this.name = source["name"];
	        this.exe = source["exe"];
	        this.command = source["command"];
	        this.cwd = source["cwd"];
	        this.user = source["user"];
	        this.uid = source["uid"];
	        this.state = source["state"];
	        this.kernelThread = source["kernelThread"];
	        this.unit = source["unit"];
	        this.cpu = source["cpu"];
	        this.cpuUser = source["cpuUser"];
	        this.cpuSystem = source["cpuSystem"];
	        this.memory = source["memory"];
	        this.memoryKind = source["memoryKind"];
	        this.rss = source["rss"];
	        this.rssAnon = source["rssAnon"];
	        this.rssFile = source["rssFile"];
	        this.rssShmem = source["rssShmem"];
	        this.swap = source["swap"];
	        this.virtual = source["virtual"];
	        this.diskRead = source["diskRead"];
	        this.diskWrite = source["diskWrite"];
	        this.diskReadTotal = source["diskReadTotal"];
	        this.diskWriteTotal = source["diskWriteTotal"];
	        this.gpu = source["gpu"];
	        this.gpuMemory = source["gpuMemory"];
	        this.gpuIndex = source["gpuIndex"];
	        this.connections = source["connections"];
	        this.energy = source["energy"];
	        this.energyScore = source["energyScore"];
	        this.threads = source["threads"];
	        this.fds = source["fds"];
	        this.nice = source["nice"];
	        this.priority = source["priority"];
	        this.ctxSwitches = source["ctxSwitches"];
	        this.oomScore = source["oomScore"];
	        this.lastCpu = source["lastCpu"];
	        this.windows = this.convertValues(source["windows"], WindowRef);
	    }

		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}
	export class WindowRef {
	    id: string;
	    title: string;
	    appId: string;
	    workspace: string;
	    output: string;
	    focused: boolean;

	    static createFrom(source: any = {}) {
	        return new WindowRef(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.id = source["id"];
	        this.title = source["title"];
	        this.appId = source["appId"];
	        this.workspace = source["workspace"];
	        this.output = source["output"];
	        this.focused = source["focused"];
	    }
	}
	export class ProcessGroup {
	    key: string;
	    category: string;
	    name: string;
	    subtitle: string;
	    iconKey?: string;
	    unit?: string;
	    leaderPid: number;
	    instances: number;
	    windows: WindowRef[];
	    state: string;
	    cpu: number;
	    memory: number;
	    memoryKind: string;
	    diskRead?: number;
	    diskWrite?: number;
	    gpu?: number;
	    gpuMemory?: number;
	    connections?: number;
	    energy: string;
	    energyScore: number;
	    threads: number;
	    members: ProcessInfo[];

	    static createFrom(source: any = {}) {
	        return new ProcessGroup(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.key = source["key"];
	        this.category = source["category"];
	        this.name = source["name"];
	        this.subtitle = source["subtitle"];
	        this.iconKey = source["iconKey"];
	        this.unit = source["unit"];
	        this.leaderPid = source["leaderPid"];
	        this.instances = source["instances"];
	        this.windows = this.convertValues(source["windows"], WindowRef);
	        this.state = source["state"];
	        this.cpu = source["cpu"];
	        this.memory = source["memory"];
	        this.memoryKind = source["memoryKind"];
	        this.diskRead = source["diskRead"];
	        this.diskWrite = source["diskWrite"];
	        this.gpu = source["gpu"];
	        this.gpuMemory = source["gpuMemory"];
	        this.connections = source["connections"];
	        this.energy = source["energy"];
	        this.energyScore = source["energyScore"];
	        this.threads = source["threads"];
	        this.members = this.convertValues(source["members"], ProcessInfo);
	    }

		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}

	export class SystemInfo {
	    hostname: string;
	    os: string;
	    kernel: string;
	    cpuModel: string;
	    logicalCpus: number;
	    physicalCores: number;

	    static createFrom(source: any = {}) {
	        return new SystemInfo(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.hostname = source["hostname"];
	        this.os = source["os"];
	        this.kernel = source["kernel"];
	        this.cpuModel = source["cpuModel"];
	        this.logicalCpus = source["logicalCpus"];
	        this.physicalCores = source["physicalCores"];
	    }
	}
	export class ThermalSensor {
	    id: string;
	    chip: string;
	    label: string;
	    temperature: number;
	    max?: number;
	    critical?: number;

	    static createFrom(source: any = {}) {
	        return new ThermalSensor(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.id = source["id"];
	        this.chip = source["chip"];
	        this.label = source["label"];
	        this.temperature = source["temperature"];
	        this.max = source["max"];
	        this.critical = source["critical"];
	    }
	}
	export class ThermalInfo {
	    hotspot?: ThermalSensor;
	    sensors: ThermalSensor[];

	    static createFrom(source: any = {}) {
	        return new ThermalInfo(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.hotspot = this.convertValues(source["hotspot"], ThermalSensor);
	        this.sensors = this.convertValues(source["sensors"], ThermalSensor);
	    }

		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}
	export class Snapshot {
	    timestampMs: number;
	    sampleMs: number;
	    accent: string;
	    windowsSource: string;
	    selfPid: number;
	    cpu: CpuInfo;
	    memory: MemoryInfo;
	    disks: DiskInfo[];
	    networks: NetworkInfo[];
	    gpus: GpuInfo[];
	    energy: EnergyInfo;
	    thermal: ThermalInfo;
	    processCount: number;
	    groups: ProcessGroup[];
	    system: SystemInfo;
	    limits: Record<string, string>;

	    static createFrom(source: any = {}) {
	        return new Snapshot(source);
	    }

	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.timestampMs = source["timestampMs"];
	        this.sampleMs = source["sampleMs"];
	        this.accent = source["accent"];
	        this.windowsSource = source["windowsSource"];
	        this.selfPid = source["selfPid"];
	        this.cpu = this.convertValues(source["cpu"], CpuInfo);
	        this.memory = this.convertValues(source["memory"], MemoryInfo);
	        this.disks = this.convertValues(source["disks"], DiskInfo);
	        this.networks = this.convertValues(source["networks"], NetworkInfo);
	        this.gpus = this.convertValues(source["gpus"], GpuInfo);
	        this.energy = this.convertValues(source["energy"], EnergyInfo);
	        this.thermal = this.convertValues(source["thermal"], ThermalInfo);
	        this.processCount = source["processCount"];
	        this.groups = this.convertValues(source["groups"], ProcessGroup);
	        this.system = this.convertValues(source["system"], SystemInfo);
	        this.limits = source["limits"];
	    }

		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}




}

