import net from 'node:net';
import dgram from 'node:dgram';

if (process.argv[2] === 'udp') {
  const socket = dgram.createSocket('udp4');
  socket.once('error', error => { throw error; });
  socket.bind(0, '127.0.0.1', () => {
    console.info(socket.address().port);
    socket.close();
  });
} else {
  const server = net.createServer();
  server.once('error', error => { throw error; });
  server.listen(0, '127.0.0.1', () => {
    const address = server.address();
    if (address === null || typeof address === 'string') throw new TypeError('Expected an IP listener');
    console.info(address.port);
    server.close();
  });
}
