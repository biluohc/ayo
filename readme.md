
# Ayo 
A high-performance web framework written in Rust. 

## Status
It is a toy now, I will write it in winter vacation(Move to tokio and futures).


## performance 
CPU: Intel Core i7-4710MQ CPU @ 8x 3.5GHz @ 8G RAM

```sh
 D/cache wrk -t 9 -c1000 http://127.0.0.1:8000/
Running 10s test @ http://127.0.0.1:8000/
  9 threads and 1000 connections
  Thread Stats   Avg      Stdev     Max   +/- Stdev
    Latency     2.36ms    3.26ms  41.31ms   88.93%
    Req/Sec    63.92k    11.50k  167.06k    71.12%
  5714800 requests in 10.10s, 773.91MB read
Requests/sec: 565924.04
Transfer/sec:     76.64MB                         (10s 198ms) 
# 46% cpu used(wrk and os used the other %50+) 
 
 
 D/cache wrk -t 1 -c1000 http://127.0.0.1:8000/                    
Running 10s test @ http://127.0.0.1:8000/
  1 threads and 1000 connections
  Thread Stats   Avg      Stdev     Max   +/- Stdev
    Latency     2.94ms  694.26us  12.72ms   65.54%
    Req/Sec   173.80k     7.22k  181.82k    76.77%
  1728717 requests in 10.09s, 234.11MB read
Requests/sec: 171357.33
Transfer/sec:     23.21MB
 D/cache                                           (10s 103ms)
 # 16% cpu used(wrk used %12) 
 ```