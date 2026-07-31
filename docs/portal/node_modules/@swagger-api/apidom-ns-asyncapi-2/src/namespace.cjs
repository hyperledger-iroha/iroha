"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _AsyncApi = _interopRequireDefault(require("./elements/AsyncApi2.cjs"));
var _AsyncApiVersion = _interopRequireDefault(require("./elements/AsyncApiVersion.cjs"));
var _ChannelBindings = _interopRequireDefault(require("./elements/ChannelBindings.cjs"));
var _ChannelItem = _interopRequireDefault(require("./elements/ChannelItem.cjs"));
var _Channels = _interopRequireDefault(require("./elements/Channels.cjs"));
var _Components = _interopRequireDefault(require("./elements/Components.cjs"));
var _Contact = _interopRequireDefault(require("./elements/Contact.cjs"));
var _CorrelationID = _interopRequireDefault(require("./elements/CorrelationID.cjs"));
var _DefaultContentType = _interopRequireDefault(require("./elements/DefaultContentType.cjs"));
var _ExternalDocumentation = _interopRequireDefault(require("./elements/ExternalDocumentation.cjs"));
var _Identifier = _interopRequireDefault(require("./elements/Identifier.cjs"));
var _Info = _interopRequireDefault(require("./elements/Info.cjs"));
var _License = _interopRequireDefault(require("./elements/License.cjs"));
var _Message = _interopRequireDefault(require("./elements/Message.cjs"));
var _MessageBindings = _interopRequireDefault(require("./elements/MessageBindings.cjs"));
var _MessageExample = _interopRequireDefault(require("./elements/MessageExample.cjs"));
var _MessageTrait = _interopRequireDefault(require("./elements/MessageTrait.cjs"));
var _OAuthFlow = _interopRequireDefault(require("./elements/OAuthFlow.cjs"));
var _OAuthFlows = _interopRequireDefault(require("./elements/OAuthFlows.cjs"));
var _Operation = _interopRequireDefault(require("./elements/Operation.cjs"));
var _OperationBindings = _interopRequireDefault(require("./elements/OperationBindings.cjs"));
var _OperationTrait = _interopRequireDefault(require("./elements/OperationTrait.cjs"));
var _Parameter = _interopRequireDefault(require("./elements/Parameter.cjs"));
var _Parameters = _interopRequireDefault(require("./elements/Parameters.cjs"));
var _Reference = _interopRequireDefault(require("./elements/Reference.cjs"));
var _Schema = _interopRequireDefault(require("./elements/Schema.cjs"));
var _SecurityRequirement = _interopRequireDefault(require("./elements/SecurityRequirement.cjs"));
var _SecurityScheme = _interopRequireDefault(require("./elements/SecurityScheme.cjs"));
var _Server = _interopRequireDefault(require("./elements/Server.cjs"));
var _ServerBindings = _interopRequireDefault(require("./elements/ServerBindings.cjs"));
var _Servers = _interopRequireDefault(require("./elements/Servers.cjs"));
var _ServerVariable = _interopRequireDefault(require("./elements/ServerVariable.cjs"));
var _Tag = _interopRequireDefault(require("./elements/Tag.cjs"));
var _Tags = _interopRequireDefault(require("./elements/Tags.cjs"));
var _AmqpChannelBinding = _interopRequireDefault(require("./elements/bindings/amqp/AmqpChannelBinding.cjs"));
var _AmqpMessageBinding = _interopRequireDefault(require("./elements/bindings/amqp/AmqpMessageBinding.cjs"));
var _AmqpOperationBinding = _interopRequireDefault(require("./elements/bindings/amqp/AmqpOperationBinding.cjs"));
var _AmqpServerBinding = _interopRequireDefault(require("./elements/bindings/amqp/AmqpServerBinding.cjs"));
var _Amqp1ChannelBinding = _interopRequireDefault(require("./elements/bindings/amqp1/Amqp1ChannelBinding.cjs"));
var _Amqp1MessageBinding = _interopRequireDefault(require("./elements/bindings/amqp1/Amqp1MessageBinding.cjs"));
var _Amqp1OperationBinding = _interopRequireDefault(require("./elements/bindings/amqp1/Amqp1OperationBinding.cjs"));
var _Amqp1ServerBinding = _interopRequireDefault(require("./elements/bindings/amqp1/Amqp1ServerBinding.cjs"));
var _AnypointmqChannelBinding = _interopRequireDefault(require("./elements/bindings/anypointmq/AnypointmqChannelBinding.cjs"));
var _AnypointmqMessageBinding = _interopRequireDefault(require("./elements/bindings/anypointmq/AnypointmqMessageBinding.cjs"));
var _AnypointmqOperationBinding = _interopRequireDefault(require("./elements/bindings/anypointmq/AnypointmqOperationBinding.cjs"));
var _AnypointmqServerBinding = _interopRequireDefault(require("./elements/bindings/anypointmq/AnypointmqServerBinding.cjs"));
var _GooglepubsubChannelBinding = _interopRequireDefault(require("./elements/bindings/googlepubsub/GooglepubsubChannelBinding.cjs"));
var _GooglepubsubMessageBinding = _interopRequireDefault(require("./elements/bindings/googlepubsub/GooglepubsubMessageBinding.cjs"));
var _GooglepubsubOperationBinding = _interopRequireDefault(require("./elements/bindings/googlepubsub/GooglepubsubOperationBinding.cjs"));
var _GooglepubsubServerBinding = _interopRequireDefault(require("./elements/bindings/googlepubsub/GooglepubsubServerBinding.cjs"));
var _HttpChannelBinding = _interopRequireDefault(require("./elements/bindings/http/HttpChannelBinding.cjs"));
var _HttpMessageBinding = _interopRequireDefault(require("./elements/bindings/http/HttpMessageBinding.cjs"));
var _HttpOperationBinding = _interopRequireDefault(require("./elements/bindings/http/HttpOperationBinding.cjs"));
var _HttpServerBinding = _interopRequireDefault(require("./elements/bindings/http/HttpServerBinding.cjs"));
var _IbmmqChannelBinding = _interopRequireDefault(require("./elements/bindings/ibmmq/IbmmqChannelBinding.cjs"));
var _IbmmqMessageBinding = _interopRequireDefault(require("./elements/bindings/ibmmq/IbmmqMessageBinding.cjs"));
var _IbmmqOperationBinding = _interopRequireDefault(require("./elements/bindings/ibmmq/IbmmqOperationBinding.cjs"));
var _IbmmqServerBinding = _interopRequireDefault(require("./elements/bindings/ibmmq/IbmmqServerBinding.cjs"));
var _JmsChannelBinding = _interopRequireDefault(require("./elements/bindings/jms/JmsChannelBinding.cjs"));
var _JmsMessageBinding = _interopRequireDefault(require("./elements/bindings/jms/JmsMessageBinding.cjs"));
var _JmsOperationBinding = _interopRequireDefault(require("./elements/bindings/jms/JmsOperationBinding.cjs"));
var _JmsServerBinding = _interopRequireDefault(require("./elements/bindings/jms/JmsServerBinding.cjs"));
var _KafkaChannelBinding = _interopRequireDefault(require("./elements/bindings/kafka/KafkaChannelBinding.cjs"));
var _KafkaMessageBinding = _interopRequireDefault(require("./elements/bindings/kafka/KafkaMessageBinding.cjs"));
var _KafkaOperationBinding = _interopRequireDefault(require("./elements/bindings/kafka/KafkaOperationBinding.cjs"));
var _KafkaServerBinding = _interopRequireDefault(require("./elements/bindings/kafka/KafkaServerBinding.cjs"));
var _MercureChannelBinding = _interopRequireDefault(require("./elements/bindings/mercure/MercureChannelBinding.cjs"));
var _MercureMessageBinding = _interopRequireDefault(require("./elements/bindings/mercure/MercureMessageBinding.cjs"));
var _MercureOperationBinding = _interopRequireDefault(require("./elements/bindings/mercure/MercureOperationBinding.cjs"));
var _MercureServerBinding = _interopRequireDefault(require("./elements/bindings/mercure/MercureServerBinding.cjs"));
var _MqttChannelBinding = _interopRequireDefault(require("./elements/bindings/mqtt/MqttChannelBinding.cjs"));
var _MqttMessageBinding = _interopRequireDefault(require("./elements/bindings/mqtt/MqttMessageBinding.cjs"));
var _MqttOperationBinding = _interopRequireDefault(require("./elements/bindings/mqtt/MqttOperationBinding.cjs"));
var _MqttServerBinding = _interopRequireDefault(require("./elements/bindings/mqtt/MqttServerBinding.cjs"));
var _Mqtt5ChannelBinding = _interopRequireDefault(require("./elements/bindings/mqtt5/Mqtt5ChannelBinding.cjs"));
var _Mqtt5MessageBinding = _interopRequireDefault(require("./elements/bindings/mqtt5/Mqtt5MessageBinding.cjs"));
var _Mqtt5OperationBinding = _interopRequireDefault(require("./elements/bindings/mqtt5/Mqtt5OperationBinding.cjs"));
var _Mqtt5ServerBinding = _interopRequireDefault(require("./elements/bindings/mqtt5/Mqtt5ServerBinding.cjs"));
var _NatsChannelBinding = _interopRequireDefault(require("./elements/bindings/nats/NatsChannelBinding.cjs"));
var _NatsMessageBinding = _interopRequireDefault(require("./elements/bindings/nats/NatsMessageBinding.cjs"));
var _NatsOperationBinding = _interopRequireDefault(require("./elements/bindings/nats/NatsOperationBinding.cjs"));
var _NatsServerBinding = _interopRequireDefault(require("./elements/bindings/nats/NatsServerBinding.cjs"));
var _PulsarChannelBinding = _interopRequireDefault(require("./elements/bindings/pulsar/PulsarChannelBinding.cjs"));
var _PulsarMessageBinding = _interopRequireDefault(require("./elements/bindings/pulsar/PulsarMessageBinding.cjs"));
var _PulsarOperationBinding = _interopRequireDefault(require("./elements/bindings/pulsar/PulsarOperationBinding.cjs"));
var _PulsarServerBinding = _interopRequireDefault(require("./elements/bindings/pulsar/PulsarServerBinding.cjs"));
var _RedisChannelBinding = _interopRequireDefault(require("./elements/bindings/redis/RedisChannelBinding.cjs"));
var _RedisMessageBinding = _interopRequireDefault(require("./elements/bindings/redis/RedisMessageBinding.cjs"));
var _RedisOperationBinding = _interopRequireDefault(require("./elements/bindings/redis/RedisOperationBinding.cjs"));
var _RedisServerBinding = _interopRequireDefault(require("./elements/bindings/redis/RedisServerBinding.cjs"));
var _SnsChannelBinding = _interopRequireDefault(require("./elements/bindings/sns/SnsChannelBinding.cjs"));
var _SnsMessageBinding = _interopRequireDefault(require("./elements/bindings/sns/SnsMessageBinding.cjs"));
var _SnsOperationBinding = _interopRequireDefault(require("./elements/bindings/sns/SnsOperationBinding.cjs"));
var _SnsServerBinding = _interopRequireDefault(require("./elements/bindings/sns/SnsServerBinding.cjs"));
var _SolaceChannelBinding = _interopRequireDefault(require("./elements/bindings/solace/SolaceChannelBinding.cjs"));
var _SolaceMessageBinding = _interopRequireDefault(require("./elements/bindings/solace/SolaceMessageBinding.cjs"));
var _SolaceOperationBinding = _interopRequireDefault(require("./elements/bindings/solace/SolaceOperationBinding.cjs"));
var _SolaceServerBinding = _interopRequireDefault(require("./elements/bindings/solace/SolaceServerBinding.cjs"));
var _SqsChannelBinding = _interopRequireDefault(require("./elements/bindings/sqs/SqsChannelBinding.cjs"));
var _SqsMessageBinding = _interopRequireDefault(require("./elements/bindings/sqs/SqsMessageBinding.cjs"));
var _SqsOperationBinding = _interopRequireDefault(require("./elements/bindings/sqs/SqsOperationBinding.cjs"));
var _SqsServerBinding = _interopRequireDefault(require("./elements/bindings/sqs/SqsServerBinding.cjs"));
var _StompChannelBinding = _interopRequireDefault(require("./elements/bindings/stomp/StompChannelBinding.cjs"));
var _StompMessageBinding = _interopRequireDefault(require("./elements/bindings/stomp/StompMessageBinding.cjs"));
var _StompOperationBinding = _interopRequireDefault(require("./elements/bindings/stomp/StompOperationBinding.cjs"));
var _StompServerBinding = _interopRequireDefault(require("./elements/bindings/stomp/StompServerBinding.cjs"));
var _WebSocketChannelBinding = _interopRequireDefault(require("./elements/bindings/ws/WebSocketChannelBinding.cjs"));
var _WebSocketMessageBinding = _interopRequireDefault(require("./elements/bindings/ws/WebSocketMessageBinding.cjs"));
var _WebSocketOperationBinding = _interopRequireDefault(require("./elements/bindings/ws/WebSocketOperationBinding.cjs"));
var _WebSocketServerBinding = _interopRequireDefault(require("./elements/bindings/ws/WebSocketServerBinding.cjs"));
/**
 * AsyncApi >= 2.0.0 <=2.6.0 specification elements.
 */

/**
 * Binding elements.
 */
// AMQP 0-9-1

// AMQP 1.0

// Anypoint MQ

// Google Cloud Pub/Sub

// HTTP

// IBM MQ

// JMS

// Kafka

// Mercure

// MQTT

// MQTT 5

// NATS

// Pulsar

// Redis

// SNS

// Solace

// SQS

// STOMP

// WebSocket

/**
 * @public
 */
const asyncApi2 = {
  namespace: options => {
    const {
      base
    } = options;

    /**
     * AsyncApi >= 2.0.0 <=2.6.0 specification elements.
     */
    base.register('asyncApi2', _AsyncApi.default);
    base.register('asyncApiVersion', _AsyncApiVersion.default);
    base.register('channelBindings', _ChannelBindings.default);
    base.register('channelItem', _ChannelItem.default);
    base.register('channels', _Channels.default);
    base.register('components', _Components.default);
    base.register('contact', _Contact.default);
    base.register('correlationID', _CorrelationID.default);
    base.register('defaultContentType', _DefaultContentType.default);
    base.register('externalDocumentation', _ExternalDocumentation.default);
    base.register('identifier', _Identifier.default);
    base.register('info', _Info.default);
    base.register('license', _License.default);
    base.register('message', _Message.default);
    base.register('messageBindings', _MessageBindings.default);
    base.register('messageExample', _MessageExample.default);
    base.register('messageTrait', _MessageTrait.default);
    base.register('oAuthFlow', _OAuthFlow.default);
    base.register('oAuthFlows', _OAuthFlows.default);
    base.register('operation', _Operation.default);
    base.register('operationBindings', _OperationBindings.default);
    base.register('operationTrait', _OperationTrait.default);
    base.register('parameter', _Parameter.default);
    base.register('parameters', _Parameters.default);
    base.register('reference', _Reference.default);
    base.register('schema', _Schema.default);
    base.register('securityRequirement', _SecurityRequirement.default);
    base.register('securityScheme', _SecurityScheme.default);
    base.register('server', _Server.default);
    base.register('serverBindings', _ServerBindings.default);
    base.register('servers', _Servers.default);
    base.register('serverVariable', _ServerVariable.default);
    base.register('tag', _Tag.default);
    base.register('tags', _Tags.default);

    /**
     * Binding elements.
     */
    // AMQP 0-9-1
    base.register('amqpChannelBinding', _AmqpChannelBinding.default);
    base.register('amqpMessageBinding', _AmqpMessageBinding.default);
    base.register('amqpOperationBinding', _AmqpOperationBinding.default);
    base.register('amqpServerBinding', _AmqpServerBinding.default);
    // AMQP 1.0
    base.register('amqp1ChannelBinding', _Amqp1ChannelBinding.default);
    base.register('amqp1MessageBinding', _Amqp1MessageBinding.default);
    base.register('amqp1OperationBinding', _Amqp1OperationBinding.default);
    base.register('amqp1ServerBinding', _Amqp1ServerBinding.default);
    // Anypoint MQ
    base.register('anypointmqChannelBinding', _AnypointmqChannelBinding.default);
    base.register('anypointmqMessageBinding', _AnypointmqMessageBinding.default);
    base.register('anypointmqOperationBinding', _AnypointmqOperationBinding.default);
    base.register('anypointmqServerBinding', _AnypointmqServerBinding.default);
    // Google Cloud Pub/Sub
    base.register('googlepubsubChannelBinding', _GooglepubsubChannelBinding.default);
    base.register('googlepubsubMessageBinding', _GooglepubsubMessageBinding.default);
    base.register('googlepubsubOperationBinding', _GooglepubsubOperationBinding.default);
    base.register('googlepubsubServerBinding', _GooglepubsubServerBinding.default);
    // HTTP
    base.register('httpChannelBinding', _HttpChannelBinding.default);
    base.register('httpMessageBinding', _HttpMessageBinding.default);
    base.register('httpOperationBinding', _HttpOperationBinding.default);
    base.register('httpServerBinding', _HttpServerBinding.default);
    // IBM MQ
    base.register('ibmmqChannelBinding', _IbmmqChannelBinding.default);
    base.register('ibmmqMessageBinding', _IbmmqMessageBinding.default);
    base.register('ibmmqOperationBinding', _IbmmqOperationBinding.default);
    base.register('ibmmqServerBinding', _IbmmqServerBinding.default);
    // JMS
    base.register('jmsChannelBinding', _JmsChannelBinding.default);
    base.register('jmsMessageBinding', _JmsMessageBinding.default);
    base.register('jmsOperationBinding', _JmsOperationBinding.default);
    base.register('jmsServerBinding', _JmsServerBinding.default);
    // Kafka
    base.register('kafkaChannelBinding', _KafkaChannelBinding.default);
    base.register('kafkaMessageBinding', _KafkaMessageBinding.default);
    base.register('kafkaOperationBinding', _KafkaOperationBinding.default);
    base.register('kafkaServerBinding', _KafkaServerBinding.default);
    // Mercure
    base.register('mercureChannelBinding', _MercureChannelBinding.default);
    base.register('mercureMessageBinding', _MercureMessageBinding.default);
    base.register('mercureOperationBinding', _MercureOperationBinding.default);
    base.register('mercureServerBinding', _MercureServerBinding.default);
    // MQTT
    base.register('mqttChannelBinding', _MqttChannelBinding.default);
    base.register('mqttMessageBinding', _MqttMessageBinding.default);
    base.register('mqttOperationBinding', _MqttOperationBinding.default);
    base.register('mqttServerBinding', _MqttServerBinding.default);
    // MQTT 5
    base.register('mqtt5ChannelBinding', _Mqtt5ChannelBinding.default);
    base.register('mqtt5MessageBinding', _Mqtt5MessageBinding.default);
    base.register('mqtt5OperationBinding', _Mqtt5OperationBinding.default);
    base.register('mqtt5ServerBinding', _Mqtt5ServerBinding.default);
    // NATS
    base.register('natsChannelBinding', _NatsChannelBinding.default);
    base.register('natsMessageBinding', _NatsMessageBinding.default);
    base.register('natsOperationBinding', _NatsOperationBinding.default);
    base.register('natsServerBinding', _NatsServerBinding.default);
    // Pulsar
    base.register('pulsarChannelBinding', _PulsarChannelBinding.default);
    base.register('pulsarMessageBinding', _PulsarMessageBinding.default);
    base.register('pulsarOperationBinding', _PulsarOperationBinding.default);
    base.register('pulsarServerBinding', _PulsarServerBinding.default);
    // Redis
    base.register('redisChannelBinding', _RedisChannelBinding.default);
    base.register('redisMessageBinding', _RedisMessageBinding.default);
    base.register('redisOperationBinding', _RedisOperationBinding.default);
    base.register('redisServerBinding', _RedisServerBinding.default);
    // SNS
    base.register('snsChannelBinding', _SnsChannelBinding.default);
    base.register('snsMessageBinding', _SnsMessageBinding.default);
    base.register('snsOperationBinding', _SnsOperationBinding.default);
    base.register('snsServerBinding', _SnsServerBinding.default);
    // Solace
    base.register('solaceChannelBinding', _SolaceChannelBinding.default);
    base.register('solaceMessageBinding', _SolaceMessageBinding.default);
    base.register('solaceOperationBinding', _SolaceOperationBinding.default);
    base.register('solaceServerBinding', _SolaceServerBinding.default);
    // SQS
    base.register('sqsChannelBinding', _SqsChannelBinding.default);
    base.register('sqsMessageBinding', _SqsMessageBinding.default);
    base.register('sqsOperationBinding', _SqsOperationBinding.default);
    base.register('sqsServerBinding', _SqsServerBinding.default);
    // STOMP
    base.register('stompChannelBinding', _StompChannelBinding.default);
    base.register('stompMessageBinding', _StompMessageBinding.default);
    base.register('stompOperationBinding', _StompOperationBinding.default);
    base.register('stompServerBinding', _StompServerBinding.default);
    // WebSocket
    base.register('webSocketChannelBinding', _WebSocketChannelBinding.default);
    base.register('webSocketMessageBinding', _WebSocketMessageBinding.default);
    base.register('webSocketOperationBinding', _WebSocketOperationBinding.default);
    base.register('webSocketServerBinding', _WebSocketServerBinding.default);
    return base;
  }
};
var _default = exports.default = asyncApi2;