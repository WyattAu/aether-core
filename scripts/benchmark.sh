#!/bin/bash
# Benchmark Runner Script

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_section() {
    echo -e "\n${BLUE}========================================${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}========================================${NC}\n"
}

BENCHMARK_TYPE=${1:-"all"}
OUTPUT_DIR=${2:-"benchmark-results"}
BASELINE=${3:-""}

mkdir -p "$OUTPUT_DIR"

run_performance_benchmarks() {
    log_section "Performance Benchmarks"
    
    log_info "Running Criterion benchmarks..."
    
    local bench_args=""
    if [ -n "$BASELINE" ]; then
        bench_args="--baseline $BASELINE"
        log_info "Comparing against baseline: $BASELINE"
    fi
    
    cargo bench --workspace $bench_args 2>&1 | tee "$OUTPUT_DIR/performance-bench.txt"
    
    log_info "Performance benchmarks complete"
    
    if [ -d "target/criterion" ]; then
        cp -r target/criterion "$OUTPUT_DIR/"
        log_info "Criterion results saved to $OUTPUT_DIR/criterion/"
    fi
}

run_memory_benchmark() {
    log_section "Memory Benchmarks"
    
    log_info "Building release binary..."
    cargo build --release
    
    if ! command -v valgrind >/dev/null 2>&1; then
        log_warn "Valgrind not found, skipping memory benchmark"
        return
    fi
    
    log_info "Running memory benchmark with Valgrind..."
    
    local binary="./target/release/aether"
    if [ -f "$binary" ]; then
        valgrind --tool=massif --massif-out-file="$OUTPUT_DIR/massif.out" $binary --help || true
        
        if [ -f "$OUTPUT_DIR/massif.out" ]; then
            ms_print "$OUTPUT_DIR/massif.out" > "$OUTPUT_DIR/memory-report.txt"
            log_info "Memory report saved to $OUTPUT_DIR/memory-report.txt"
            
            echo -e "\n${GREEN}Memory Usage Summary:${NC}"
            head -30 "$OUTPUT_DIR/memory-report.txt"
        fi
    else
        log_warn "Binary not found, skipping memory benchmark"
    fi
}

run_startup_benchmark() {
    log_section "Startup Time Benchmarks"
    
    log_info "Building release binary..."
    cargo build --release
    
    local binary="./target/release/aether"
    if [ ! -f "$binary" ]; then
        log_warn "Binary not found, skipping startup benchmark"
        return
    fi
    
    log_info "Measuring startup time (10 runs)..."
    
    echo "Run,Time (ms)" > "$OUTPUT_DIR/startup-times.csv"
    
    local total=0
    for i in {1..10}; do
        start=$(date +%s%N)
        $binary --help > /dev/null 2>&1
        end=$(date +%s%N)
        time_ms=$(( (end - start) / 1000000 ))
        total=$((total + time_ms))
        echo "$i,$time_ms" >> "$OUTPUT_DIR/startup-times.csv"
        echo "  Run $i: ${time_ms}ms"
    done
    
    local avg=$((total / 10))
    echo -e "\n${GREEN}Average startup time: ${avg}ms${NC}"
    
    echo "Average,$avg" >> "$OUTPUT_DIR/startup-times.csv"
}

run_throughput_benchmark() {
    log_section "Throughput Benchmarks"
    
    if [ -f "benches/throughput.rs" ]; then
        log_info "Running throughput benchmarks..."
        cargo bench --bench throughput 2>&1 | tee "$OUTPUT_DIR/throughput-bench.txt"
    else
        log_warn "Throughput benchmark not found, skipping"
    fi
}

run_latency_benchmark() {
    log_section "Latency Benchmarks"
    
    if [ -f "benches/latency.rs" ]; then
        log_info "Running latency benchmarks..."
        cargo bench --bench latency 2>&1 | tee "$OUTPUT_DIR/latency-bench.txt"
    else
        log_warn "Latency benchmark not found, skipping"
    fi
}

save_baseline() {
    log_section "Saving Baseline"
    
    if [ -d "target/criterion" ]; then
        local baseline_dir="baselines/$(date +%Y%m%d-%H%M%S)"
        mkdir -p "$baseline_dir"
        cp -r target/criterion "$baseline_dir/"
        log_info "Baseline saved to $baseline_dir/"
    else
        log_warn "No criterion results to save"
    fi
}

generate_report() {
    log_section "Generating Benchmark Report"
    
    local report_file="$OUTPUT_DIR/benchmark-report.md"
    
    echo "# Benchmark Report" > "$report_file"
    echo "" >> "$report_file"
    echo "Generated: $(date)" >> "$report_file"
    echo "" >> "$report_file"
    
    if [ -f "$OUTPUT_DIR/performance-bench.txt" ]; then
        echo "## Performance Benchmarks" >> "$report_file"
        echo '```' >> "$report_file"
        cat "$OUTPUT_DIR/performance-bench.txt" >> "$report_file"
        echo '```' >> "$report_file"
        echo "" >> "$report_file"
    fi
    
    if [ -f "$OUTPUT_DIR/memory-report.txt" ]; then
        echo "## Memory Usage" >> "$report_file"
        echo '```' >> "$report_file"
        cat "$OUTPUT_DIR/memory-report.txt" >> "$report_file"
        echo '```' >> "$report_file"
        echo "" >> "$report_file"
    fi
    
    if [ -f "$OUTPUT_DIR/startup-times.csv" ]; then
        echo "## Startup Times" >> "$report_file"
        echo '```csv' >> "$report_file"
        cat "$OUTPUT_DIR/startup-times.csv" >> "$report_file"
        echo '```' >> "$report_file"
        echo "" >> "$report_file"
    fi
    
    log_info "Report generated: $report_file"
}

main() {
    log_info "🏃 Starting benchmark suite"
    log_info "Type: $BENCHMARK_TYPE"
    log_info "Output directory: $OUTPUT_DIR"
    
    case "$BENCHMARK_TYPE" in
        "performance")
            run_performance_benchmarks
            ;;
        "memory")
            run_memory_benchmark
            ;;
        "startup")
            run_startup_benchmark
            ;;
        "throughput")
            run_throughput_benchmark
            ;;
        "latency")
            run_latency_benchmark
            ;;
        "all")
            run_performance_benchmarks
            run_memory_benchmark
            run_startup_benchmark
            run_throughput_benchmark
            run_latency_benchmark
            ;;
        *)
            log_error "Unknown benchmark type: $BENCHMARK_TYPE"
            echo "Usage: $0 [performance|memory|startup|throughput|latency|all] [output_dir] [baseline]"
            exit 1
            ;;
    esac
    
    generate_report
    
    log_info "✅ Benchmark suite completed successfully!"
}

main
