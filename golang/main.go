package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"log"
	"net/http"
	"os"
	"sort"
	"strconv"
	"time"
)

type RPCRequest struct {
	JSONrpc string      `json:"jsonrpc"`
	ID      int         `json:"id"`
	Method  string      `json:"method"`
	Params  interface{} `json:"params,omitempty"`
}

type RPCResponse struct {
	JSONrpc string      `json:"jsonrpc"`
	ID      int         `json:"id"`
	Result  interface{} `json:"result,omitempty"`
	Error   interface{} `json:"error,omitempty"`
}

type TestResult struct {
	Method  string      `json:"method"`
	Success bool        `json:"success"`
	Latency int64       `json:"latency"`
	Result  interface{} `json:"result,omitempty"`
	Error   string      `json:"error,omitempty"`
}

type BenchmarkStats struct {
	TotalRequests      int     `json:"totalRequests"`
	SuccessfulRequests int     `json:"successfulRequests"`
	FailedRequests     int     `json:"failedRequests"`
	SuccessRate        float64 `json:"successRate"`
	Latency            struct {
		Avg float64 `json:"avg"`
		Min int64   `json:"min"`
		Max int64   `json:"max"`
		P50 int64   `json:"p50"`
		P95 int64   `json:"p95"`
		P99 int64   `json:"p99"`
	} `json:"latency"`
}

type SolanaRPCTester struct {
	Endpoint string
	Client   *http.Client
}

func NewSolanaRPCTester(endpoint string) *SolanaRPCTester {
	return &SolanaRPCTester{
		Endpoint: endpoint,
		Client: &http.Client{
			Timeout: 30 * time.Second,
		},
	}
}

func (s *SolanaRPCTester) makeRPCCall(method string, params interface{}) (*TestResult, error) {
	start := time.Now()

	request := RPCRequest{
		JSONrpc: "2.0",
		ID:      1,
		Method:  method,
		Params:  params,
	}

	jsonData, err := json.Marshal(request)
	if err != nil {
		return &TestResult{
			Method:  method,
			Success: false,
			Latency: time.Since(start).Milliseconds(),
			Error:   err.Error(),
		}, nil
	}

	resp, err := s.Client.Post(s.Endpoint, "application/json", bytes.NewBuffer(jsonData))
	if err != nil {
		return &TestResult{
			Method:  method,
			Success: false,
			Latency: time.Since(start).Milliseconds(),
			Error:   err.Error(),
		}, nil
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return &TestResult{
			Method:  method,
			Success: false,
			Latency: time.Since(start).Milliseconds(),
			Error:   err.Error(),
		}, nil
	}

	var rpcResponse RPCResponse
	err = json.Unmarshal(body, &rpcResponse)
	if err != nil {
		return &TestResult{
			Method:  method,
			Success: false,
			Latency: time.Since(start).Milliseconds(),
			Error:   err.Error(),
		}, nil
	}

	latency := time.Since(start).Milliseconds()

	if rpcResponse.Error != nil {
		return &TestResult{
			Method:  method,
			Success: false,
			Latency: latency,
			Error:   fmt.Sprintf("%v", rpcResponse.Error),
		}, nil
	}

	return &TestResult{
		Method:  method,
		Success: true,
		Latency: latency,
		Result:  rpcResponse.Result,
	}, nil
}

func (s *SolanaRPCTester) TestGetVersion() (*TestResult, error) {
	return s.makeRPCCall("getVersion", nil)
}

func (s *SolanaRPCTester) TestGetSlot() (*TestResult, error) {
	return s.makeRPCCall("getSlot", nil)
}

func (s *SolanaRPCTester) TestGetBalance(publicKey string) (*TestResult, error) {
	params := []interface{}{publicKey}
	return s.makeRPCCall("getBalance", params)
}

func (s *SolanaRPCTester) RunBenchmark(iterations int) (*BenchmarkStats, error) {
	fmt.Printf("Running Go RPC benchmark with %d iterations...\n", iterations)
	
	var results []TestResult

	for i := 0; i < iterations; i++ {
		versionResult, err := s.TestGetVersion()
		if err != nil {
			return nil, err
		}
		
		slotResult, err := s.TestGetSlot()
		if err != nil {
			return nil, err
		}

		results = append(results, *versionResult, *slotResult)

		if (i+1)%10 == 0 {
			fmt.Printf("Completed %d/%d iterations\n", i+1, iterations)
		}
	}

	return s.calculateStats(results), nil
}

func (s *SolanaRPCTester) calculateStats(results []TestResult) *BenchmarkStats {
	var latencies []int64
	successfulRequests := 0

	for _, result := range results {
		if result.Success {
			successfulRequests++
			latencies = append(latencies, result.Latency)
		}
	}

	if len(latencies) == 0 {
		return &BenchmarkStats{
			TotalRequests:      len(results),
			SuccessfulRequests: 0,
			FailedRequests:     len(results),
			SuccessRate:        0,
		}
	}

	sort.Slice(latencies, func(i, j int) bool {
		return latencies[i] < latencies[j]
	})

	var sum int64
	for _, latency := range latencies {
		sum += latency
	}

	stats := &BenchmarkStats{
		TotalRequests:      len(results),
		SuccessfulRequests: successfulRequests,
		FailedRequests:     len(results) - successfulRequests,
		SuccessRate:        float64(successfulRequests) / float64(len(results)) * 100,
	}

	stats.Latency.Avg = float64(sum) / float64(len(latencies))
	stats.Latency.Min = latencies[0]
	stats.Latency.Max = latencies[len(latencies)-1]
	stats.Latency.P50 = latencies[int(float64(len(latencies))*0.5)]
	stats.Latency.P95 = latencies[int(float64(len(latencies))*0.95)]
	stats.Latency.P99 = latencies[int(float64(len(latencies))*0.99)]

	return stats
}

func main() {
	endpoint := "https://api.mainnet-beta.solana.com"
	iterations := 100

	if len(os.Args) > 1 {
		endpoint = os.Args[1]
	}
	if len(os.Args) > 2 {
		if i, err := strconv.Atoi(os.Args[2]); err == nil {
			iterations = i
		}
	}

	tester := NewSolanaRPCTester(endpoint)
	
	stats, err := tester.RunBenchmark(iterations)
	if err != nil {
		log.Fatal(err)
	}

	fmt.Println("\n=== Go RPC Performance Results ===")
	statsJSON, err := json.MarshalIndent(stats, "", "  ")
	if err != nil {
		log.Fatal(err)
	}
	fmt.Println(string(statsJSON))
}