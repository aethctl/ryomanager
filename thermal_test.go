package main

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func writeThermalFixture(t *testing.T, path, value string) {
	t.Helper()

	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte(value), 0o644); err != nil {
		t.Fatal(err)
	}
}

func TestReadThermalsInReturnsEmptyArray(t *testing.T) {
	root := t.TempDir()

	got := readThermalsIn(
		filepath.Join(root, "thermal"),
		filepath.Join(root, "hwmon"),
	)

	if got.Sensors == nil {
		t.Fatal("expected empty thermal sensor array, got nil")
	}

	payload, err := json.Marshal(got)
	if err != nil {
		t.Fatal(err)
	}

	if !strings.Contains(string(payload), `"sensors":[]`) {
		t.Fatalf("thermal sensors did not serialize as an empty array: %s", payload)
	}
}

func TestReadThermalsInReadsHwmon(t *testing.T) {
	root := t.TempDir()

	thermalRoot := filepath.Join(root, "thermal")
	hwmonRoot := filepath.Join(root, "hwmon")
	hwmon := filepath.Join(hwmonRoot, "hwmon0")

	writeThermalFixture(t, filepath.Join(hwmon, "name"), "k10temp\n")
	writeThermalFixture(t, filepath.Join(hwmon, "temp1_label"), "Tctl\n")
	writeThermalFixture(t, filepath.Join(hwmon, "temp1_input"), "63500\n")
	writeThermalFixture(t, filepath.Join(hwmon, "temp1_max"), "90000\n")
	writeThermalFixture(t, filepath.Join(hwmon, "temp1_crit"), "95000\n")

	writeThermalFixture(t, filepath.Join(thermalRoot, "thermal_zone0", "type"), "acpitz\n")
	writeThermalFixture(t, filepath.Join(thermalRoot, "thermal_zone0", "temp"), "42000\n")

	got := readThermalsIn(thermalRoot, hwmonRoot)

	if len(got.Sensors) != 1 {
		t.Fatalf("expected hwmon sensors to be preferred, got %d sensors", len(got.Sensors))
	}

	sensor := got.Sensors[0]

	if sensor.Chip != "k10temp" {
		t.Fatalf("chip = %q, want k10temp", sensor.Chip)
	}
	if sensor.Label != "Tctl" {
		t.Fatalf("label = %q, want Tctl", sensor.Label)
	}
	if sensor.Temperature != 63.5 {
		t.Fatalf("temperature = %v, want 63.5", sensor.Temperature)
	}
	if sensor.Max == nil || *sensor.Max != 90 {
		t.Fatalf("max = %v, want 90", sensor.Max)
	}
	if sensor.Critical == nil || *sensor.Critical != 95 {
		t.Fatalf("critical = %v, want 95", sensor.Critical)
	}
}

func TestNormalizeSnapshotCollections(t *testing.T) {
	snapshot := Snapshot{}

	normalizeSnapshotCollections(&snapshot)

	if snapshot.CPU.Cores == nil {
		t.Fatal("CPU cores remained nil")
	}
	if snapshot.CPU.Caches == nil {
		t.Fatal("CPU caches remained nil")
	}
	if snapshot.Disks == nil {
		t.Fatal("disks remained nil")
	}
	if snapshot.Networks == nil {
		t.Fatal("networks remained nil")
	}
	if snapshot.GPUs == nil {
		t.Fatal("GPUs remained nil")
	}
	if snapshot.Thermal.Sensors == nil {
		t.Fatal("thermal sensors remained nil")
	}
	if snapshot.Groups == nil {
		t.Fatal("groups remained nil")
	}
}
