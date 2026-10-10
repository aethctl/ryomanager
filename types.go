package main

type Availability map[string]string

type WindowRef struct {
	ID        string `json:"id"`
	Title     string `json:"title"`
	AppID     string `json:"appId"`
	Workspace string `json:"workspace"`
	Output    string `json:"output"`
	Focused   bool   `json:"focused"`
}

type ProcessInfo struct {
	PID            int         `json:"pid"`
	ParentPID      *int        `json:"parentPid"`
	StartTime      uint64      `json:"startTime"`
	Name           string      `json:"name"`
	Exe            *string     `json:"exe"`
	Command        string      `json:"command"`
	Cwd            *string     `json:"cwd"`
	User           string      `json:"user"`
	UID            uint32      `json:"uid"`
	State          string      `json:"state"`
	KernelThread   bool        `json:"kernelThread"`
	Unit           *string     `json:"unit"`
	CPU            float64     `json:"cpu"`
	CPUUser        float64     `json:"cpuUser"`
	CPUSystem      float64     `json:"cpuSystem"`
	Memory         uint64      `json:"memory"`
	MemoryKind     string      `json:"memoryKind"`
	RSS            uint64      `json:"rss"`
	RSSAnon        *uint64     `json:"rssAnon"`
	RSSFile        *uint64     `json:"rssFile"`
	RSSShmem       *uint64     `json:"rssShmem"`
	Swap           *uint64     `json:"swap"`
	Virtual        uint64      `json:"virtual"`
	DiskRead       *float64    `json:"diskRead"`
	DiskWrite      *float64    `json:"diskWrite"`
	DiskReadTotal  *uint64     `json:"diskReadTotal"`
	DiskWriteTotal *uint64     `json:"diskWriteTotal"`
	GPU            *float64    `json:"gpu"`
	GPUMemory      *uint64     `json:"gpuMemory"`
	GPUIndex       *uint32     `json:"gpuIndex"`
	Connections    *uint64     `json:"connections"`
	Energy         string      `json:"energy"`
	EnergyScore    float64     `json:"energyScore"`
	Threads        uint64      `json:"threads"`
	FDs            *uint64     `json:"fds"`
	Nice           int64       `json:"nice"`
	Priority       int64       `json:"priority"`
	CtxSwitches    *uint64     `json:"ctxSwitches"`
	OOMScore       *int64      `json:"oomScore"`
	LastCPU        *uint32     `json:"lastCpu"`
	Windows        []WindowRef `json:"windows"`
}

type ProcessGroup struct {
	Key         string        `json:"key"`
	Category    string        `json:"category"`
	Name        string        `json:"name"`
	Subtitle    string        `json:"subtitle"`
	IconKey     *string       `json:"iconKey"`
	Unit        *string       `json:"unit"`
	LeaderPID   int           `json:"leaderPid"`
	Instances   uint64        `json:"instances"`
	Windows     []WindowRef   `json:"windows"`
	State       string        `json:"state"`
	CPU         float64       `json:"cpu"`
	Memory      uint64        `json:"memory"`
	MemoryKind  string        `json:"memoryKind"`
	DiskRead    *float64      `json:"diskRead"`
	DiskWrite   *float64      `json:"diskWrite"`
	GPU         *float64      `json:"gpu"`
	GPUMemory   *uint64       `json:"gpuMemory"`
	Connections *uint64       `json:"connections"`
	Energy      string        `json:"energy"`
	EnergyScore float64       `json:"energyScore"`
	Threads     uint64        `json:"threads"`
	Members     []ProcessInfo `json:"members"`
}

type Pressure struct {
	Some10  float64  `json:"some10"`
	Some60  float64  `json:"some60"`
	Some300 float64  `json:"some300"`
	Full10  *float64 `json:"full10"`
	Full60  *float64 `json:"full60"`
	Full300 *float64 `json:"full300"`
}

type CpuCore struct {
	Index   uint32   `json:"index"`
	Usage   float64  `json:"usage"`
	FreqMhz *float64 `json:"freqMhz"`
	Kind    *string  `json:"kind"`
}
type CpuCache struct {
	Level string `json:"level"`
	Size  string `json:"size"`
}
type CpuInfo struct {
	Usage          float64    `json:"usage"`
	User           float64    `json:"user"`
	System         float64    `json:"system"`
	IOWait         float64    `json:"iowait"`
	IRQ            float64    `json:"irq"`
	Cores          []CpuCore  `json:"cores"`
	FreqMhz        *float64   `json:"freqMhz"`
	MaxFreqMhz     *float64   `json:"maxFreqMhz"`
	Governor       *string    `json:"governor"`
	LoadAvg        [3]float64 `json:"loadAvg"`
	UptimeSeconds  float64    `json:"uptimeSeconds"`
	Processes      uint64     `json:"processes"`
	Threads        uint64     `json:"threads"`
	OpenFiles      *uint64    `json:"openFiles"`
	OpenFilesMax   *uint64    `json:"openFilesMax"`
	Model          string     `json:"model"`
	Sockets        uint64     `json:"sockets"`
	PhysicalCores  uint64     `json:"physicalCores"`
	LogicalCPUs    uint64     `json:"logicalCpus"`
	Virtualization *string    `json:"virtualization"`
	Caches         []CpuCache `json:"caches"`
	Pressure       *Pressure  `json:"pressure"`
}
type MemoryInfo struct {
	Total       uint64       `json:"total"`
	Used        uint64       `json:"used"`
	Available   uint64       `json:"available"`
	Free        uint64       `json:"free"`
	Buffers     uint64       `json:"buffers"`
	Cached      uint64       `json:"cached"`
	Shared      uint64       `json:"shared"`
	Dirty       uint64       `json:"dirty"`
	Mapped      uint64       `json:"mapped"`
	Committed   uint64       `json:"committed"`
	CommitLimit uint64       `json:"commitLimit"`
	SwapTotal   uint64       `json:"swapTotal"`
	SwapUsed    uint64       `json:"swapUsed"`
	SwapCached  uint64       `json:"swapCached"`
	Zswap       *uint64      `json:"zswap"`
	Pressure    *Pressure    `json:"pressure"`
	Limits      Availability `json:"limits"`
}
type DiskMount struct {
	Path   string `json:"path"`
	FS     string `json:"fs"`
	Used   uint64 `json:"used"`
	Total  uint64 `json:"total"`
	System bool   `json:"system"`
}
type DiskInfo struct {
	Name          string      `json:"name"`
	Model         *string     `json:"model"`
	Kind          string      `json:"kind"`
	Capacity      uint64      `json:"capacity"`
	ReadRate      float64     `json:"readRate"`
	WriteRate     float64     `json:"writeRate"`
	ReadIops      float64     `json:"readIops"`
	WriteIops     float64     `json:"writeIops"`
	ActivePercent float64     `json:"activePercent"`
	ResponseMs    *float64    `json:"responseMs"`
	ReadTotal     uint64      `json:"readTotal"`
	WriteTotal    uint64      `json:"writeTotal"`
	Mounts        []DiskMount `json:"mounts"`
}
type NetworkInfo struct {
	Name         string   `json:"name"`
	Kind         string   `json:"kind"`
	State        string   `json:"state"`
	RxRate       float64  `json:"rxRate"`
	TxRate       float64  `json:"txRate"`
	RxTotal      uint64   `json:"rxTotal"`
	TxTotal      uint64   `json:"txTotal"`
	IPv4         []string `json:"ipv4"`
	IPv6         []string `json:"ipv6"`
	MAC          *string  `json:"mac"`
	MTU          *uint64  `json:"mtu"`
	SpeedMbps    *float64 `json:"speedMbps"`
	SSID         *string  `json:"ssid"`
	Signal       *float64 `json:"signal"`
	FrequencyMhz *float64 `json:"frequencyMhz"`
	Driver       *string  `json:"driver"`
}
type GpuInfo struct {
	Index          uint32       `json:"index"`
	Name           string       `json:"name"`
	Vendor         string       `json:"vendor"`
	Usage          *float64     `json:"usage"`
	MemoryUsage    *float64     `json:"memoryUsage"`
	Encoder        *float64     `json:"encoder"`
	Decoder        *float64     `json:"decoder"`
	MemoryUsed     *uint64      `json:"memoryUsed"`
	MemoryTotal    *uint64      `json:"memoryTotal"`
	Temperature    *float64     `json:"temperature"`
	Power          *float64     `json:"power"`
	PowerLimit     *float64     `json:"powerLimit"`
	ClockMhz       *float64     `json:"clockMhz"`
	MemoryClockMhz *float64     `json:"memoryClockMhz"`
	Driver         *string      `json:"driver"`
	Pstate         *string      `json:"pstate"`
	Limits         Availability `json:"limits"`
}
type EnergyInfo struct {
	Source              string       `json:"source"`
	BatteryPercent      *float64     `json:"batteryPercent"`
	BatteryStatus       *string      `json:"batteryStatus"`
	BatteryPower        *float64     `json:"batteryPower"`
	BatteryEnergyNow    *float64     `json:"batteryEnergyNow"`
	BatteryEnergyFull   *float64     `json:"batteryEnergyFull"`
	BatteryEnergyDesign *float64     `json:"batteryEnergyDesign"`
	TimeToEmptySeconds  *float64     `json:"timeToEmptySeconds"`
	CycleCount          *uint64      `json:"cycleCount"`
	PackagePower        *float64     `json:"packagePower"`
	Limits              Availability `json:"limits"`
}
type ThermalSensor struct {
	ID          string   `json:"id"`
	Chip        string   `json:"chip"`
	Label       string   `json:"label"`
	Temperature float64  `json:"temperature"`
	Max         *float64 `json:"max"`
	Critical    *float64 `json:"critical"`
}
type ThermalInfo struct {
	Hotspot *ThermalSensor  `json:"hotspot"`
	Sensors []ThermalSensor `json:"sensors"`
}
type SystemInfo struct {
	Hostname      string `json:"hostname"`
	OS            string `json:"os"`
	Kernel        string `json:"kernel"`
	CPUModel      string `json:"cpuModel"`
	LogicalCPUs   uint64 `json:"logicalCpus"`
	PhysicalCores uint64 `json:"physicalCores"`
}
type Snapshot struct {
	TimestampMS   int64          `json:"timestampMs"`
	SampleMS      float64        `json:"sampleMs"`
	Accent        string         `json:"accent"`
	WindowsSource string         `json:"windowsSource"`
	SelfPID       int            `json:"selfPid"`
	CPU           CpuInfo        `json:"cpu"`
	Memory        MemoryInfo     `json:"memory"`
	Disks         []DiskInfo     `json:"disks"`
	Networks      []NetworkInfo  `json:"networks"`
	GPUs          []GpuInfo      `json:"gpus"`
	Energy        EnergyInfo     `json:"energy"`
	Thermal       ThermalInfo    `json:"thermal"`
	ProcessCount  int            `json:"processCount"`
	Groups        []ProcessGroup `json:"groups"`
	System        SystemInfo     `json:"system"`
	Limits        Availability   `json:"limits"`
}
