package com.ppaass.common.handler;

import com.ppaass.common.log.IPpaassLogger;
import com.ppaass.common.log.PpaassLoggerFactory;
import io.netty.channel.ChannelHandler;
import io.netty.channel.ChannelHandlerContext;
import io.netty.channel.ChannelInboundHandlerAdapter;

@ChannelHandler.Sharable
public class PrintExceptionHandler extends ChannelInboundHandlerAdapter {
    private final IPpaassLogger logger = PpaassLoggerFactory.INSTANCE.getLogger();
    public static final PrintExceptionHandler INSTANCE = new PrintExceptionHandler();

    private PrintExceptionHandler() {
    }

    @Override
    public void exceptionCaught(ChannelHandlerContext ctx, Throwable cause) throws Exception {
        logger
                .trace(PrintExceptionHandler.class, () -> "Exception in the channel pipeline, channel = {}",
                        () -> new Object[]{
                                ctx.channel().id(), cause
                        });
        ctx.fireExceptionCaught(cause);
    }
}


