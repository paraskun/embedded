`timescale 1ns / 1ps

module cnt4(
    input              clk,
    input              res,
    input              ud,
    output logic [3:0] q
    );

    enum logic [1:0] { ST0, ST1, ST2, ST3 } state = ST0;

    always_ff @(posedge clk) begin
        case (state)
            ST0: if (res == 1'b1) state <= ST0;
                 else if (ud == 1'b1) state <= ST1;
                 else state <= ST3;
            ST1: if (res == 1'b1) state <= ST0;
                 else if (ud == 1'b1) state <= ST2;
                 else state <= ST0;
            ST2: if (res == 1'b1) state <= ST0;
                 else if (ud == 1'b1) state <= ST3;
                 else state <= ST1;
            ST3: if (res == 1'b1) state <= ST0;
                 else if (ud == 1'b1) state <= ST0;
                 else state <= ST2;
        endcase
    end

    assign q[0] = (state == ST0);
    assign q[1] = (state == ST1);
    assign q[2] = (state == ST2);
    assign q[3] = (state == ST3);
endmodule