export namespace main {
	
	export class ProcessInfo {
	    pid: number;
	    parentPid?: number;
	    name: string;
	    command: string;
	    status: string;
	    cpu: number;
	    memory: number;
	
	    static createFrom(source: any = {}) {
	        return new ProcessInfo(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.pid = source["pid"];
	        this.parentPid = source["parentPid"];
	        this.name = source["name"];
	        this.command = source["command"];
	        this.status = source["status"];
	        this.cpu = source["cpu"];
	        this.memory = source["memory"];
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
	export class Snapshot {
	    timestampMs: number;
	    accent: string;
	    cpu: number;
	    totalMemory: number;
	    usedMemory: number;
	    totalSwap: number;
	    usedSwap: number;
	    processCount: number;
	    processes: ProcessInfo[];
	    system: SystemInfo;
	
	    static createFrom(source: any = {}) {
	        return new Snapshot(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.timestampMs = source["timestampMs"];
	        this.accent = source["accent"];
	        this.cpu = source["cpu"];
	        this.totalMemory = source["totalMemory"];
	        this.usedMemory = source["usedMemory"];
	        this.totalSwap = source["totalSwap"];
	        this.usedSwap = source["usedSwap"];
	        this.processCount = source["processCount"];
	        this.processes = this.convertValues(source["processes"], ProcessInfo);
	        this.system = this.convertValues(source["system"], SystemInfo);
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

