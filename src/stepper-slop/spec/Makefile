QUINT ?= quint
PYTHON ?= python3
LMT ?= lmt
SAMPLES ?= 10000
STEPS ?= 300
WITNESSES = queued dmaWrote dmaCompleted reclaimed sealed started pulled cycled stalled stallTicked idled abortBegan dmaQuiesced smStopped abortFinished resetHappened dmaDoneMotionActive pulseHighReached completedCommand

.PHONY: extract format dev-extract dev-format typecheck test sample service check dev-check

extract:
	$(LMT) stepper.md

# Official gate: clean extraction, repeat extraction, generated-file drift.
format:
	$(PYTHON) check_extract.py --lmt $(LMT)

# Development fallback; explicitly NOT the official lmt gate.
dev-extract:
	$(PYTHON) dev_extract.py

dev-format:
	$(PYTHON) check_extract.py --development

typecheck:
	$(QUINT) typecheck stepper.qnt
	$(QUINT) typecheck stepper_test.qnt

test:
	$(QUINT) test stepper_test.qnt --max-samples $(SAMPLES) --seed 1 --verbosity 1

sample:
	$(QUINT) run stepper.qnt --invariant safety --witnesses $(WITNESSES) --max-samples $(SAMPLES) --max-steps $(STEPS) --seed 1 --verbosity 1

# Fair-service scenarios are not a liveness proof. This action excludes abort.
service:
	$(QUINT) run stepper.qnt --step serviceStep --invariant safety --witnesses stalled stallTicked idled completedCommand pulseHighReached --max-samples $(SAMPLES) --max-steps $(STEPS) --seed 1 --verbosity 1

check: extract format typecheck test sample service
dev-check: dev-extract dev-format typecheck test sample service
