IVERILOG     := iverilog -g2012
VCD2LXT      := vcd2lxt
GTKWAVE      := gtkwave
GTKWAVE_FIFO := out/gtkwave.fifo

PROJECTS := cmp2 cmp4 cmp8 mux2 mux4 mux8 mux16 coder decoder pr_coder

.PHONY: all clean $(PROJECTS)

all: $(PROJECTS)

clean:
	rm -rf out

# $(1) = project name, $(2) = source files (deps first, then own module, then tb)
define sim_rules
$(1): out/$(1)/sim.lxt

out/$(1)/sim.out: $(2)
	@mkdir -p out/$(1)
	$(IVERILOG) -o $$@ $$^

out/$(1)/sim.lxt: out/$(1)/sim.out
	./$$<
	$(VCD2LXT) out/$(1)/sim.vcd $$@

.PHONY: wave-$(1)
wave-$(1): out/$(1)/sim.lxt
	@if pgrep -f gtkwave.tcl > /dev/null 2>&1; then \
	    echo 'gtkwave::loadFile "$(CURDIR)/out/$(1)/sim.lxt"' > $(GTKWAVE_FIFO); \
	else \
	    mkfifo $(GTKWAVE_FIFO) 2>/dev/null || true; \
	    setsid $(GTKWAVE) --script gtkwave.tcl out/$(1)/sim.lxt & \
	fi
endef

$(eval $(call sim_rules,cmp2,   cmp2/cmp2.sv cmp2/tb_cmp2.sv))
$(eval $(call sim_rules,cmp4,   cmp2/cmp2.sv cmp4/cmp4.sv cmp4/tb_cmp4.sv))
$(eval $(call sim_rules,cmp8,   cmp2/cmp2.sv cmp8/cmp8.sv cmp8/tb_cmp8.sv))
$(eval $(call sim_rules,mux2,   mux2/mux2.sv mux2/tb_mux2.sv))
$(eval $(call sim_rules,mux4,   mux4/mux4.sv mux4/tb_mux4.sv))
$(eval $(call sim_rules,mux8,   mux8/mux8.sv mux8/tb_mux8.sv))
$(eval $(call sim_rules,mux16,  mux4/mux4.sv mux16/mux16.sv mux16/tb_mux16.sv))
$(eval $(call sim_rules,coder,  coder/coder.sv coder/tb_coder.sv))
$(eval $(call sim_rules,decoder,decoder/decoder.sv decoder/tb_decoder.sv))
$(eval $(call sim_rules,pr_coder,pr_coder/pr_coder.sv pr_coder/tb_pr_coder.sv))
$(eval $(call sim_rules,sum,    sum/sum.sv sum/tb_sum.sv))
$(eval $(call sim_rules,sum4,   sum/sum.sv sum/cla4.sv sum/sum4.sv sum/tb_sum4.sv))
$(eval $(call sim_rules,sum16,  sum/sum.sv sum/cla4.sv sum/sum4.sv sum/sum16.sv sum/tb_sum16.sv))
$(eval $(call sim_rules,sum64,  sum/sum.sv sum/cla4.sv sum/sum4.sv sum/sum16.sv sum/sum64.sv sum/tb_sum64.sv))
$(eval $(call sim_rules,sum64x3,  sum/sum.sv sum/cla4.sv sum/sum4.sv sum/sum16.sv sum/sum64.sv sum/sum64x3.sv sum/tb_sum64x3.sv))
