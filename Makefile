.DEFAULT_GOAL := help
RPS      ?= 5000
DURATION ?= 60
SERVICES := order-service billing-service search-sync metrics-gateway

.PHONY: help up down logs ps load load-local test fmt lint grafana-json \
        kind-up kind-images kind-deploy kind-down kind-load

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## ' $(MAKEFILE_LIST) | awk 'BEGIN{FS=":.*?## "}{printf "  \033[36m%-14s\033[0m %s\n",$$1,$$2}'

# ───────────── docker compose ─────────────
up: ## Build and start the whole stack
	@test -f .env || cp .env.example .env
	docker compose up -d --build
	@echo "dashboard http://localhost:8080 | grafana http://localhost:3001 | rabbitmq http://localhost:15672"

down: ## Stop and delete all data
	docker compose --profile load down -v

logs: ## Tail service logs
	docker compose logs -f order-1 billing-service search-sync metrics-gateway

ps: ## Container status
	docker compose ps

load: ## Flood the edge in a container (RPS=5000 DURATION=60)
	docker compose --profile load run --rm loadgen --url http://nginx-edge:8080 --rps $(RPS) --duration $(DURATION) --ramp 10 --slo-p99-ms 5

load-local: ## Same, from the host (release build; needs `ulimit -n 65535`)
	cargo run --release -p loadgen -- --url http://localhost:8080 --rps $(RPS) --duration $(DURATION)

# ───────────── development ─────────────
test: ## Unit tests for the workspace (needs libcurl headers for librdkafka)
	cargo test --workspace

fmt: ## Format
	cargo fmt --all

lint: ## Clippy, warnings are errors
	cargo clippy --workspace --all-targets -- -D warnings

grafana-json: ## Regenerate the Grafana dashboard from scripts/gen_grafana.py
	python3 scripts/gen_grafana.py

# ───────────── kubernetes (kind) ─────────────
kind-up: ## Create the kind cluster + ingress-nginx + metrics-server
	kind create cluster --config infra/k8s/kind-cluster.yaml
	kubectl apply -f https://kind.sigs.k8s.io/examples/ingress/deploy-ingress-nginx.yaml
	kubectl apply -f https://github.com/kubernetes-sigs/metrics-server/releases/latest/download/components.yaml
	kubectl -n kube-system patch deploy metrics-server --type=json \
	  -p '[{"op":"add","path":"/spec/template/spec/containers/0/args/-","value":"--kubelet-insecure-tls"}]'
	kubectl apply -f https://github.com/kubernetes/kube-state-metrics/releases/latest/download/standard.yaml 2>/dev/null || \
	  kubectl apply -k "github.com/kubernetes/kube-state-metrics/examples/standard?ref=main"

kind-images: ## Build service images
	for s in $(SERVICES); do docker build -f docker/Dockerfile.rust --build-arg SERVICE=$$s -t pshop/$$s:dev . ; done
	docker build -f docker/Dockerfile.dashboard -t pshop/dashboard:dev dashboard

kind-load: kind-images ## Load images into the kind nodes
	for s in $(SERVICES) dashboard; do kind load docker-image pshop/$$s:dev --name pshop ; done

kind-deploy: ## Apply the kustomize overlay
	mkdir -p infra/k8s/grafana
	cp infra/grafana/provisioning/datasources/prometheus.yml infra/k8s/grafana/datasources.yml
	cp infra/grafana/provisioning/dashboards/dashboards.yml   infra/k8s/grafana/dashboards.yml
	cp infra/grafana/dashboards/pshop.json                    infra/k8s/grafana/pshop.json
	kubectl create namespace pshop --dry-run=client -o yaml | kubectl apply -f -
	kubectl -n pshop create configmap terraform-localstack --from-file=infra/terraform/localstack \
	  --dry-run=client -o yaml | kubectl apply -f -
	kubectl apply -k infra/k8s
	kubectl -n pshop rollout status deploy/order-service --timeout=300s

kind-down: ## Delete the cluster
	kind delete cluster --name pshop
