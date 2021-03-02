FROM redis:6.0.9

COPY ./conf/redis.conf /usr/local/etc/redis/redis.conf
ENTRYPOINT ["redis-server", "/usr/local/etc/redis/redis.conf"]
